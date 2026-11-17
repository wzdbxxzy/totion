//! 实验性「桌面毛玻璃侧栏」：屏幕右侧 1/3–1/2 宽的半透明面板，直接复用 TUI。
//!
//! - 整个面板是一层分层窗口（32 位 ARGB），桌面/壁纸会透出来；
//! - 内部用 ratatui 渲染真正的 TUI（今日/工作区/日历/习惯/统计/留言），
//!   键位与终端版一致，所以设置、邀请码、搜索等功能都能用；
//! - 配色映射到极简暖白 + 半透明深色，贴合浅色/动漫壁纸。
//!
//! 用法：`totion-glass [--opacity 150] [--width 38]`，`--wallpaper <图片>` 设置壁纸。

#[path = "../api.rs"]
#[allow(dead_code)]
mod api;
#[path = "../app.rs"]
#[allow(dead_code)]
mod app;
#[path = "../charts.rs"]
#[allow(dead_code)]
mod charts;
#[path = "../config.rs"]
#[allow(dead_code)]
mod config;
#[path = "../ui.rs"]
#[allow(dead_code)]
mod ui;

#[cfg(not(windows))]
fn main() {
    eprintln!("totion-glass 目前只支持 Windows。");
}

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--wallpaper" {
        imp::set_wallpaper(&args[2]);
        return;
    }
    let opacity = arg_value(&args, "--opacity").unwrap_or(150).clamp(60, 235) as u32;
    let width = arg_value(&args, "--width").unwrap_or(38).clamp(33, 50);
    imp::run(opacity, width);
}

#[cfg(windows)]
fn arg_value(args: &[String], name: &str) -> Option<i32> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
}

#[cfg(windows)]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr;

    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::{Color as TuiColor, Modifier as TuiModifier};
    use ratatui::Terminal;
    use totion_core::{Snapshot, VerifyRequest, WorkspaceInfo};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, CreateFontW, GetDC, GetTextExtentPoint32W, ReleaseDC,
        ScreenToClient, SelectObject, SetBkMode, SetTextColor, TextOutW, AC_SRC_ALPHA, AC_SRC_OVER,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET,
        DEFAULT_PITCH, DIB_RGB_COLORS, FF_DONTCARE, FW_BOLD, FW_NORMAL, HBITMAP, HDC, HFONT,
        OUT_DEFAULT_PRECIS,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
        GetWindowRect, LoadCursorW, PostQuitMessage, RegisterClassW, SetCursor,
        SetForegroundWindow, SetTimer, ShowWindow, SystemParametersInfoW, TranslateMessage,
        UpdateLayeredWindow, CS_DBLCLKS, IDC_ARROW, SW_SHOW, ULW_ALPHA, WM_CHAR, WM_CLOSE,
        WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_MOUSEWHEEL,
        WM_RBUTTONUP, WM_SETCURSOR, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_POPUP,
        WS_VISIBLE,
    };

    use crate::app::{App, Screen};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
    };

    const MARGIN: i32 = 16;
    const BG_RGB: u32 = 0x001D_1612; // #12161D
    const TIMER_REFRESH: usize = 1;
    const PANEL_RADIUS: f64 = 22.0;
    const CONTENT_MARGIN: i32 = 2; // 单元格内边距
    const WM_MOUSELEAVE_MSG: u32 = 0x02A3;

    thread_local! {
        static APP: RefCell<Option<App>> = const { RefCell::new(None) };
        static CANVAS: Cell<(isize, isize, isize, i32, i32)> =
            const { Cell::new((0, 0, 0, 1, 1)) }; // mem, bitmap, bits, cell_w, cell_h
        static FONT_NORMAL: Cell<isize> = const { Cell::new(0) };
        static FONT_BOLD: Cell<isize> = const { Cell::new(0) };
        static OPACITY: Cell<u32> = const { Cell::new(150) };
        static COLUMNS: Cell<i32> = const { Cell::new(0) };
        static ROWS: Cell<i32> = const { Cell::new(0) };
        static CONTENT: Cell<(i32, i32)> = const { Cell::new((1, 1)) };
        static HOVER: Cell<Option<(i32, i32, i32, i32)>> = const { Cell::new(None) };
        static CURSOR_ARROW: Cell<isize> = const { Cell::new(0) };
        static CURSOR_HAND: Cell<isize> = const { Cell::new(0) };
    }

    #[link(name = "user32")]
    extern "system" {
        fn SetProcessDPIAware() -> i32;
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn set_wallpaper(path: &str) -> bool {
        unsafe {
            let value = wide(path);
            let ok = SystemParametersInfoW(
                0x0014, // SPI_SETDESKWALLPAPER
                0,
                value.as_ptr() as *mut c_void,
                0x01 | 0x02,
            );
            if ok == 0 {
                eprintln!("设置壁纸失败，请确认文件路径存在且为 jpg/png/bmp");
                false
            } else {
                println!("壁纸已设置为: {path}");
                true
            }
        }
    }

    pub fn run(opacity: u32, width_percent: i32) {
        OPACITY.with(|value| value.set(opacity));
        unsafe {
            SetProcessDPIAware();
            let instance = GetModuleHandleW(ptr::null());
            let class_name = wide("TotionGlassPanel");
            let cursor = LoadCursorW(ptr::null_mut(), IDC_ARROW);

            let class = WNDCLASSW {
                style: CS_DBLCLKS,
                lpfnWndProc: Some(wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: ptr::null_mut(),
                hCursor: cursor,
                hbrBackground: ptr::null_mut(),
                lpszMenuName: ptr::null(),
                lpszClassName: class_name.as_ptr(),
            };
            RegisterClassW(&class);

            let arrow_cursor = LoadCursorW(ptr::null_mut(), IDC_ARROW);
            let hand_cursor = LoadCursorW(ptr::null_mut(), 32649 as *const u16); // IDC_HAND
            CURSOR_ARROW.with(|value| value.set(arrow_cursor as isize));
            CURSOR_HAND.with(|value| value.set(hand_cursor as isize));

            build_app();
            create_canvas(width_percent);

            let mut work = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            SystemParametersInfoW(0x0030, 0, &mut work as *mut RECT as *mut c_void, 0);
            let (cell_w, cell_h) = CANVAS.with(|value| {
                let (_, _, _, cell_w, cell_h) = value.get();
                (cell_w, cell_h)
            });
            let cols = COLUMNS.with(|value| value.get());
            let rows = ROWS.with(|value| value.get());
            let panel_w = cols * cell_w;
            let panel_h = rows * cell_h;
            let x = work.right - panel_w - MARGIN;
            let y = work.top + MARGIN;

            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW,
                class_name.as_ptr(),
                wide("totion").as_ptr(),
                WS_POPUP | WS_VISIBLE,
                x,
                y,
                panel_w,
                panel_h,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null(),
            );
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            SetFocus(hwnd);
            render(hwnd);
            SetTimer(hwnd, TIMER_REFRESH, 1000, None);

            let mut message = std::mem::zeroed();
            while GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    fn build_app() {
        let identity = crate::config::load_identity().ok().flatten();
        let (api, name, user_id, snapshot) = match identity {
            Some(identity) => {
                let mut client = crate::api::Api::new(&identity.server_url);
                let auth = (|| {
                    let challenge = client.challenge(&identity.public_key).ok()?;
                    let signature = identity.sign(challenge.nonce.as_bytes()).ok()?;
                    client
                        .verify(&VerifyRequest {
                            challenge_id: challenge.challenge_id,
                            signature,
                        })
                        .ok()
                })();
                match auth {
                    Some(auth) => {
                        client.set_token(Some(auth.token));
                        let name = auth.user.name.clone();
                        let user_id = auth.user.id.clone();
                        match client.snapshot() {
                            Ok(snapshot) => (client, name, user_id, snapshot),
                            Err(err) => {
                                eprintln!("拉取数据失败: {err}");
                                (
                                    client,
                                    identity.name.clone(),
                                    identity.user_id.clone(),
                                    empty_snapshot(),
                                )
                            }
                        }
                    }
                    None => (
                        client,
                        identity.name.clone(),
                        identity.user_id.clone(),
                        empty_snapshot(),
                    ),
                }
            }
            None => (
                crate::api::Api::new("http://127.0.0.1:1"),
                "totion".to_string(),
                "local".to_string(),
                empty_snapshot(),
            ),
        };
        let state = crate::app::MainState::from_snapshot(api, name, user_id, snapshot);
        APP.with(|value| {
            *value.borrow_mut() = Some(App {
                screen: Screen::Main(state),
                should_quit: false,
            })
        });
    }

    fn empty_snapshot() -> Snapshot {
        Snapshot {
            workspace: WorkspaceInfo {
                name: "totion".to_string(),
                users: Vec::new(),
            },
            pages: Vec::new(),
            blocks: Vec::new(),
            habits: Vec::new(),
            checkins: Vec::new(),
            comments: Vec::new(),
            journals: Vec::new(),
            last_seq: 0,
            api_version: 5,
        }
    }

    unsafe fn create_canvas(width_percent: i32) {
        let screen = GetDC(ptr::null_mut());
        let mem = CreateCompatibleDC(screen);
        ReleaseDC(ptr::null_mut(), screen);

        let normal = create_font(-15, FW_NORMAL, "NSimSun");
        let bold = create_font(-15, FW_BOLD, "NSimSun");
        FONT_NORMAL.with(|value| value.set(normal as isize));
        FONT_BOLD.with(|value| value.set(bold as isize));

        SelectObject(mem, normal);
        let mut metrics = SIZE { cx: 0, cy: 0 };
        let sample = wide("M");
        GetTextExtentPoint32W(mem, sample.as_ptr(), 1, &mut metrics);
        let cell_w = metrics.cx.max(6);
        let cell_h = metrics.cy.max(12);

        let mut work = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        SystemParametersInfoW(0x0030, 0, &mut work as *mut RECT as *mut c_void, 0);
        let work_w = work.right - work.left;
        let work_h = work.bottom - work.top;
        let target_px = work_w * width_percent / 100;
        let cols = (target_px / cell_w).max(40);
        let rows = ((work_h - MARGIN * 2) / cell_h).max(20);
        COLUMNS.with(|value| value.set(cols));
        ROWS.with(|value| value.set(rows));
        CONTENT.with(|value| {
            value.set((
                (cols - CONTENT_MARGIN * 2).max(10),
                (rows - CONTENT_MARGIN * 2).max(6),
            ))
        });

        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: cols * cell_w,
                biHeight: -(rows * cell_h),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [std::mem::zeroed()],
        };
        let mut bits = ptr::null_mut();
        let bitmap: HBITMAP =
            CreateDIBSection(mem, &info, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
        SelectObject(mem, bitmap);
        CANVAS.with(|value| {
            value.set((mem as isize, bitmap as isize, bits as isize, cell_w, cell_h))
        });
    }

    unsafe fn create_font(height: i32, weight: u32, face: &str) -> HFONT {
        let face = wide(face);
        CreateFontW(
            height,
            0,
            0,
            0,
            weight as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            4, // ANTIALIASED_QUALITY
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            face.as_ptr(),
        )
    }

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_CREATE => 0,
            WM_TIMER => {
                tick();
                render(hwnd);
                0
            }
            WM_KEYDOWN => {
                if let Some(event) = key_from_vk(wparam as u32) {
                    dispatch(event, hwnd);
                }
                0
            }
            WM_CHAR => {
                let code = wparam as u32;
                let (key, modifiers) = match code {
                    3 => (KeyCode::Char('c'), KeyModifiers::CONTROL),
                    19 => (KeyCode::Char('s'), KeyModifiers::CONTROL),
                    32..=0x10FFFF => match char::from_u32(code) {
                        Some(ch) => (KeyCode::Char(ch), KeyModifiers::NONE),
                        None => return 0,
                    },
                    _ => return 0,
                };
                dispatch(KeyEvent::new(key, modifiers), hwnd);
                0
            }
            WM_MOUSEMOVE => {
                track_leave(hwnd);
                let x = (lparam & 0xFFFF) as u16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as u16 as i32;
                update_hover(hwnd, x, y);
                0
            }
            WM_MOUSELEAVE_MSG => {
                HOVER.with(|value| value.set(None));
                set_cursor(false);
                render(hwnd);
                0
            }
            WM_SETCURSOR => {
                set_cursor(HOVER.with(|value| value.get().is_some()));
                1
            }
            WM_LBUTTONDOWN => {
                SetForegroundWindow(hwnd);
                SetFocus(hwnd);
                let x = (lparam & 0xFFFF) as u16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as u16 as i32;
                forward_mouse(hwnd, MouseEventKind::Down(MouseButton::Left), x, y);
                0
            }
            WM_MOUSEWHEEL => {
                let delta = ((wparam >> 16) & 0xFFFF) as u16 as i16;
                let mut point = POINT {
                    x: (lparam & 0xFFFF) as u16 as i32,
                    y: ((lparam >> 16) & 0xFFFF) as u16 as i32,
                };
                ScreenToClient(hwnd, &mut point);
                let kind = if delta > 0 {
                    MouseEventKind::ScrollUp
                } else {
                    MouseEventKind::ScrollDown
                };
                forward_mouse(hwnd, kind, point.x, point.y);
                0
            }
            WM_RBUTTONUP | WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    fn cell_at(x: i32, y: i32, cell_w: i32, cell_h: i32) -> (u16, u16) {
        ((x / cell_w).max(0) as u16, (y / cell_h).max(0) as u16)
    }

    fn forward_mouse(hwnd: HWND, kind: MouseEventKind, x: i32, y: i32) {
        let (cell_w, cell_h) = CANVAS.with(|value| {
            let (_, _, _, cell_w, cell_h) = value.get();
            (cell_w, cell_h)
        });
        let (column, row) = cell_at(x, y, cell_w, cell_h);
        let (content_cols, content_rows) = CONTENT.with(|value| value.get());
        let column = column as i32 - CONTENT_MARGIN;
        let row = row as i32 - CONTENT_MARGIN;
        if column < 0 || row < 0 || column >= content_cols || row >= content_rows {
            return;
        }
        let (column, row) = (column as u16, row as u16);
        let quit = APP.with(|value| {
            let mut app = value.borrow_mut();
            let Some(app) = app.as_mut() else {
                return false;
            };
            app.handle_mouse(MouseEvent {
                kind,
                column,
                row,
                modifiers: KeyModifiers::NONE,
            });
            app.should_quit
        });
        if quit {
            unsafe { DestroyWindow(hwnd) };
        } else {
            unsafe { render(hwnd) };
        }
    }

    fn set_cursor(hand: bool) {
        let cursor = if hand {
            CURSOR_HAND.with(|value| value.get())
        } else {
            CURSOR_ARROW.with(|value| value.get())
        };
        if cursor != 0 {
            unsafe { SetCursor(cursor as *mut c_void) };
        }
    }

    fn track_leave(hwnd: HWND) {
        let mut event = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: 0,
        };
        unsafe { TrackMouseEvent(&mut event) };
    }

    fn update_hover(hwnd: HWND, x: i32, y: i32) {
        let (cell_w, cell_h) = CANVAS.with(|value| {
            let (_, _, _, cell_w, cell_h) = value.get();
            (cell_w, cell_h)
        });
        let (content_cols, content_rows) = CONTENT.with(|value| value.get());
        let column = x / cell_w - CONTENT_MARGIN;
        let row = y / cell_h - CONTENT_MARGIN;
        let hovered = if column >= 0 && row >= 0 && column < content_cols && row < content_rows {
            APP.with(|value| {
                let borrowed = value.borrow();
                let app = borrowed.as_ref()?;
                match &app.screen {
                    Screen::Main(state) => state.hover_rect(column as u16, row as u16),
                    Screen::Setup(_) => None,
                }
            })
        } else {
            None
        };
        let rect = hovered.map(|rect| {
            (
                rect.x as i32 + CONTENT_MARGIN,
                rect.y as i32 + CONTENT_MARGIN,
                rect.width as i32,
                rect.height as i32,
            )
        });
        let changed = HOVER.with(|value| {
            if value.get() == rect {
                false
            } else {
                value.set(rect);
                true
            }
        });
        set_cursor(rect.is_some());
        if changed {
            unsafe { render(hwnd) };
        }
    }

    fn key_from_vk(vk: u32) -> Option<KeyEvent> {
        let code = match vk {
            0x25 => KeyCode::Left,
            0x26 => KeyCode::Up,
            0x27 => KeyCode::Right,
            0x28 => KeyCode::Down,
            0x0D => KeyCode::Enter,
            0x1B => KeyCode::Esc,
            0x09 => KeyCode::Tab,
            0x08 => KeyCode::Backspace,
            0x2E => KeyCode::Delete,
            0x24 => KeyCode::Home,
            0x23 => KeyCode::End,
            0x21 => KeyCode::PageUp,
            0x22 => KeyCode::PageDown,
            _ => return None,
        };
        Some(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn dispatch(event: KeyEvent, hwnd: HWND) {
        let quit = APP.with(|value| {
            let mut app = value.borrow_mut();
            let Some(app) = app.as_mut() else {
                return false;
            };
            app.handle_key(event);
            app.should_quit
        });
        if quit {
            unsafe { DestroyWindow(hwnd) };
        } else {
            unsafe { render(hwnd) };
        }
    }

    fn tick() {
        APP.with(|value| {
            if let Some(app) = value.borrow_mut().as_mut() {
                app.tick();
            }
        });
    }

    unsafe fn render(hwnd: HWND) {
        let (mem, _bitmap, bits, cell_w, cell_h) = CANVAS.with(|value| value.get());
        if bits == 0 {
            return;
        }
        let cols = COLUMNS.with(|value| value.get()).max(1);
        let rows = ROWS.with(|value| value.get()).max(1);
        let width = (cols * cell_w) as usize;
        let height = (rows * cell_h) as usize;
        let (content_cols, content_rows) = CONTENT.with(|value| value.get());
        let offset_x = CONTENT_MARGIN * cell_w;
        let offset_y = CONTENT_MARGIN * cell_h;

        let buffer = APP.with(|value| {
            let borrowed = value.borrow();
            let app = borrowed.as_ref()?;
            let backend = TestBackend::new(content_cols as u16, content_rows as u16);
            let mut terminal = Terminal::new(backend).ok()?;
            terminal.draw(|frame| crate::ui::draw(frame, app)).ok()?;
            Some(terminal.backend().buffer().clone())
        });
        let Some(buffer) = buffer else {
            return;
        };

        let bits = bits as *mut u32;
        let opacity = OPACITY.with(|value| value.get());
        fill_background(bits, width, height, opacity);
        paint_cell_backgrounds(
            bits, width, height, cell_w, cell_h, offset_x, offset_y, &buffer,
        );
        if let Some((x, y, w, h)) = HOVER.with(|value| value.get()) {
            paint_hover(bits, width, height, cell_w, cell_h, x, y, w, h);
        }
        let pre: Vec<u32> = std::slice::from_raw_parts(bits, width * height).to_vec();
        paint_text(mem as HDC, &buffer, cell_w, cell_h, offset_x, offset_y);
        mark_text_pixels(bits, &pre);
        present(hwnd, mem as HDC, width as i32, height as i32);
    }

    fn fill_background(bits: *mut u32, width: usize, height: usize, opacity: u32) {
        let half_w = width as f64 / 2.0;
        let half_h = height as f64 / 2.0;
        for y in 0..height {
            for x in 0..width {
                let cx = x as f64 + 0.5 - half_w;
                let cy = y as f64 + 0.5 - half_h;
                let qx = cx.abs() - (half_w - PANEL_RADIUS);
                let qy = cy.abs() - (half_h - PANEL_RADIUS);
                let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
                let distance = outside + qx.max(qy).min(0.0) - PANEL_RADIUS;
                let coverage = (0.5 - distance).clamp(0.0, 1.0);
                let depth = 0.88 + 0.12 * (1.0 - y as f64 / height as f64);
                let alpha = (opacity as f64 * coverage * depth).round() as u32;
                let pixel = if coverage > 0.0 && distance > -2.0 {
                    // 内侧一像素细边，低透明度。
                    ((70.0 * coverage) as u32) << 24 | 0x005A_4A3E
                } else {
                    (alpha << 24) | BG_RGB
                };
                unsafe {
                    *bits.add(y * width + x) = pixel;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_cell_backgrounds(
        bits: *mut u32,
        width: usize,
        height: usize,
        cell_w: i32,
        cell_h: i32,
        offset_x: i32,
        offset_y: i32,
        buffer: &Buffer,
    ) {
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let Some(cell) = buffer.cell((x, y)) else {
                    continue;
                };
                if cell.bg == TuiColor::Reset {
                    continue;
                }
                let (r, g, b) = tui_rgb(cell.bg);
                let left = offset_x + x as i32 * cell_w;
                let top = offset_y + y as i32 * cell_h;
                for py in top..(top + cell_h).min(height as i32) {
                    for px in left..(left + cell_w).min(width as i32) {
                        let index = py as usize * width + px as usize;
                        unsafe {
                            let pixel = *bits.add(index);
                            let (dr, dg, db) =
                                ((pixel >> 16) & 0xFF, (pixel >> 8) & 0xFF, pixel & 0xFF);
                            let blend = 0.72;
                            let nr = (r as f64 * blend + dr as f64 * (1.0 - blend)) as u32;
                            let ng = (g as f64 * blend + dg as f64 * (1.0 - blend)) as u32;
                            let nb = (b as f64 * blend + db as f64 * (1.0 - blend)) as u32;
                            *bits.add(index) = (210u32 << 24) | (nr << 16) | (ng << 8) | nb;
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_hover(
        bits: *mut u32,
        width: usize,
        height: usize,
        cell_w: i32,
        cell_h: i32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) {
        let left = (x * cell_w).max(0) as usize;
        let top = (y * cell_h).max(0) as usize;
        let right = ((x + w) * cell_w).min(width as i32).max(0) as usize;
        let bottom = ((y + h) * cell_h).min(height as i32).max(0) as usize;
        for py in top..bottom {
            for px in left..right {
                let index = py * width + px;
                unsafe {
                    let pixel = *bits.add(index);
                    let (dr, dg, db) = ((pixel >> 16) & 0xFF, (pixel >> 8) & 0xFF, pixel & 0xFF);
                    let blend = 0.12;
                    let nr = (255.0 * blend + dr as f64 * (1.0 - blend)) as u32;
                    let ng = (245.0 * blend + dg as f64 * (1.0 - blend)) as u32;
                    let nb = (230.0 * blend + db as f64 * (1.0 - blend)) as u32;
                    *bits.add(index) = (pixel & 0xFF00_0000) | (nr << 16) | (ng << 8) | nb;
                }
            }
        }
    }

    unsafe fn paint_text(
        hdc: HDC,
        buffer: &Buffer,
        cell_w: i32,
        cell_h: i32,
        offset_x: i32,
        offset_y: i32,
    ) {
        SetBkMode(hdc, 1); // TRANSPARENT
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let Some(cell) = buffer.cell((x, y)) else {
                    continue;
                };
                let symbol = cell.symbol();
                if symbol.is_empty() || symbol == " " {
                    continue;
                }
                let bold = cell.modifier.contains(TuiModifier::BOLD);
                let font = if bold {
                    FONT_BOLD.with(|value| value.get())
                } else {
                    FONT_NORMAL.with(|value| value.get())
                };
                if font == 0 {
                    continue;
                }
                SelectObject(hdc, font as *mut c_void);
                let (r, g, b) = tui_rgb(cell.fg);
                SetTextColor(hdc, (r as u32) | ((g as u32) << 8) | ((b as u32) << 16));
                let text = wide(symbol);
                TextOutW(
                    hdc,
                    offset_x + x as i32 * cell_w,
                    offset_y + y as i32 * cell_h,
                    text.as_ptr(),
                    text.len() as i32 - 1,
                );
            }
        }
    }

    fn mark_text_pixels(bits: *mut u32, pre: &[u32]) {
        for (index, before) in pre.iter().enumerate() {
            unsafe {
                let pixel = *bits.add(index);
                if pixel != *before {
                    *bits.add(index) = pixel | 0xFF00_0000;
                }
            }
        }
    }

    unsafe fn present(hwnd: HWND, mem: HDC, width: i32, height: i32) {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetWindowRect(hwnd, &mut rect);
        let destination = POINT {
            x: rect.left,
            y: rect.top,
        };
        let source = POINT { x: 0, y: 0 };
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let screen = GetDC(ptr::null_mut());
        UpdateLayeredWindow(
            hwnd,
            screen,
            &destination,
            &size,
            mem,
            &source,
            0,
            &blend,
            ULW_ALPHA,
        );
        ReleaseDC(ptr::null_mut(), screen);
    }

    fn tui_rgb(color: TuiColor) -> (u8, u8, u8) {
        match color {
            TuiColor::Reset | TuiColor::White => (0xF5, 0xF0, 0xEC),
            TuiColor::LightRed => (0xF0, 0xB0, 0xB0),
            TuiColor::LightGreen => (0xB8, 0xE0, 0xA8),
            TuiColor::LightYellow => (0xF0, 0xDC, 0xA0),
            TuiColor::LightBlue => (0xB0, 0xD0, 0xF0),
            TuiColor::LightMagenta => (0xF0, 0xC8, 0xDC),
            TuiColor::LightCyan => (0xF5, 0xE8, 0xDC),
            TuiColor::Black => (0x12, 0x12, 0x16),
            TuiColor::Red => (0xE8, 0x90, 0x90),
            TuiColor::Green => (0x9C, 0xCB, 0x8A),
            TuiColor::Yellow => (0xE8, 0xC8, 0x7A),
            TuiColor::Blue => (0x9A, 0xB8, 0xD8),
            TuiColor::Magenta => (0xE8, 0xA8, 0xC0),
            TuiColor::Cyan => (0xE8, 0xD8, 0xC8),
            TuiColor::Gray => (0xB6, 0xAD, 0xA6),
            TuiColor::DarkGray => (0x8E, 0x86, 0x80),
            TuiColor::Rgb(r, g, b) => (r, g, b),
            TuiColor::Indexed(_) => (0xB6, 0xAD, 0xA6),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn pixel_maps_to_cell() {
            assert_eq!(cell_at(0, 0, 7, 16), (0, 0));
            assert_eq!(cell_at(13, 31, 7, 16), (1, 1));
            assert_eq!(cell_at(-5, -5, 7, 16), (0, 0));
        }

        #[test]
        fn palette_maps_named_colors() {
            assert_eq!(tui_rgb(TuiColor::Cyan), (0xE8, 0xD8, 0xC8));
            assert_eq!(tui_rgb(TuiColor::Rgb(1, 2, 3)), (1, 2, 3));
        }
    }
}
