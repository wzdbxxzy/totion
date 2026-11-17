//! 实验性「桌面毛玻璃卡片」：把今日概览固定成一张半透明卡片。
//!
//! 使用分层窗口 + 32 位 ARGB 画布，桌面会透过卡片显示（磨砂质感）。
//! 交互：左键拖动移动，双击刷新，右键退出。仅 Windows 可用。

#[path = "../api.rs"]
#[allow(dead_code)]
mod api;
#[path = "../config.rs"]
#[allow(dead_code)]
mod config;

#[cfg(not(windows))]
fn main() {
    eprintln!("totion-glass 目前只支持 Windows。");
}

#[cfg(windows)]
fn main() {
    imp::run();
}

#[cfg(windows)]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr;

    use chrono::{Datelike, Local, NaiveDate};
    use totion_core::{habit_due_on, habit_stats, BlockKind, Snapshot};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush, DeleteObject, GetDC,
        ReleaseDC, RoundRect, SelectObject, SetBkMode, SetTextColor, TextOutW, AC_SRC_ALPHA,
        AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, CLIP_DEFAULT_PRECIS,
        DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, FF_DONTCARE, FW_BOLD, FW_NORMAL, HBITMAP,
        HDC, HFONT, OUT_DEFAULT_PRECIS,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos, GetMessageW,
        GetWindowRect, LoadCursorW, PostQuitMessage, RegisterClassW, SetTimer, SetWindowPos,
        ShowWindow, SystemParametersInfoW, TranslateMessage, UpdateLayeredWindow, CS_DBLCLKS,
        IDC_ARROW, SW_SHOW, ULW_ALPHA, WM_CREATE, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
        WM_LBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONUP, WM_TIMER, WNDCLASSW, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
    };

    const WIDTH: i32 = 460;
    const HEIGHT: i32 = 330;
    const PADDING: i32 = 22;
    const RADIUS: i32 = 22;
    const TIMER_REFRESH: usize = 1;

    // 背景与文字颜色（ARGB / COLORREF 0x00BBGGRR）。
    const BG_RGB: u32 = 0x001D_1612; // #12161D
    const BG_ALPHA: u32 = 208;
    const BORDER_RGB: u32 = 0x0050_4438; // #384450

    thread_local! {
        static SNAPSHOT: RefCell<Option<Snapshot>> = const { RefCell::new(None) };
        static ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
        static FONT_TITLE: Cell<isize> = const { Cell::new(0) };
        static FONT_BODY: Cell<isize> = const { Cell::new(0) };
        static FONT_SMALL: Cell<isize> = const { Cell::new(0) };
        static DRAGGING: Cell<bool> = const { Cell::new(false) };
        static DRAG_ORIGIN: Cell<(POINT, POINT)> =
            const { Cell::new((POINT { x: 0, y: 0 }, POINT { x: 0, y: 0 })) };
        static CANVAS: Cell<(isize, isize, isize)> = const { Cell::new((0, 0, 0)) };
    }

    #[link(name = "user32")]
    extern "system" {
        fn SetProcessDPIAware() -> i32;
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn run() {
        unsafe {
            SetProcessDPIAware();
            let instance = GetModuleHandleW(ptr::null());
            let class_name = wide("TotionGlass");
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

            let mut work = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            SystemParametersInfoW(
                0x0030, // SPI_GETWORKAREA
                0,
                &mut work as *mut RECT as *mut c_void,
                0,
            );
            let x = work.right - WIDTH - 32;
            let y = work.bottom - HEIGHT - 32;

            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class_name.as_ptr(),
                wide("totion").as_ptr(),
                WS_POPUP | WS_VISIBLE,
                x,
                y,
                WIDTH,
                HEIGHT,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null(),
            );
            ShowWindow(hwnd, SW_SHOW);

            let mut message = std::mem::zeroed();
            while GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_CREATE => {
                create_resources();
                refresh(hwnd);
                SetTimer(hwnd, TIMER_REFRESH, 60_000, None);
                0
            }
            WM_TIMER => {
                refresh(hwnd);
                0
            }
            WM_LBUTTONDOWN => {
                let mut cursor = POINT { x: 0, y: 0 };
                let mut rect = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                GetCursorPos(&mut cursor);
                GetWindowRect(hwnd, &mut rect);
                DRAGGING.with(|value| value.set(true));
                DRAG_ORIGIN.with(|value| {
                    value.set((
                        cursor,
                        POINT {
                            x: rect.left,
                            y: rect.top,
                        },
                    ))
                });
                0
            }
            WM_MOUSEMOVE if DRAGGING.with(|value| value.get()) => {
                let mut cursor = POINT { x: 0, y: 0 };
                GetCursorPos(&mut cursor);
                let (origin, window) = DRAG_ORIGIN.with(|value| value.get());
                SetWindowPos(
                    hwnd,
                    ptr::null_mut(),
                    window.x + cursor.x - origin.x,
                    window.y + cursor.y - origin.y,
                    0,
                    0,
                    0x0001 | 0x0004, // SWP_NOSIZE | SWP_NOZORDER
                );
                0
            }
            WM_LBUTTONUP => {
                DRAGGING.with(|value| value.set(false));
                0
            }
            WM_LBUTTONDBLCLK => {
                refresh(hwnd);
                0
            }
            WM_RBUTTONUP => {
                PostQuitMessage(0);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    unsafe fn create_resources() {
        let screen = GetDC(ptr::null_mut());
        let mem = CreateCompatibleDC(screen);
        ReleaseDC(ptr::null_mut(), screen);

        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: WIDTH,
                biHeight: -HEIGHT,
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
        CANVAS.with(|value| value.set((mem as isize, bitmap as isize, bits as isize)));

        let title = create_font(-24, FW_BOLD, "Microsoft YaHei UI");
        let body = create_font(-16, FW_NORMAL, "Microsoft YaHei UI");
        let small = create_font(-13, FW_NORMAL, "Microsoft YaHei UI");
        FONT_TITLE.with(|value| value.set(title as isize));
        FONT_BODY.with(|value| value.set(body as isize));
        FONT_SMALL.with(|value| value.set(small as isize));
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

    fn refresh(hwnd: HWND) {
        match fetch() {
            Ok(snapshot) => {
                SNAPSHOT.with(|value| *value.borrow_mut() = Some(snapshot));
                ERROR.with(|value| *value.borrow_mut() = None);
            }
            Err(err) => ERROR.with(|value| *value.borrow_mut() = Some(err)),
        }
        unsafe { render(hwnd) };
    }

    fn fetch() -> Result<Snapshot, String> {
        let identity = crate::config::load_identity()
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "还没有连接过服务器，请先运行 totion 完成设置".to_string())?;
        let mut client = crate::api::Api::new(&identity.server_url);
        let challenge = client
            .challenge(&identity.public_key)
            .map_err(|err| err.to_string())?;
        let signature = identity
            .sign(challenge.nonce.as_bytes())
            .map_err(|err| err.to_string())?;
        let auth = client
            .verify(&totion_core::VerifyRequest {
                challenge_id: challenge.challenge_id,
                signature,
            })
            .map_err(|err| err.to_string())?;
        client.set_token(Some(auth.token));
        client.snapshot().map_err(|err| err.to_string())
    }

    /// 画一张圆角半透明卡片到 ARGB 画布，再交给 UpdateLayeredWindow。
    unsafe fn render(hwnd: HWND) {
        let (mem, _bitmap, bits) = CANVAS.with(|value| value.get());
        if bits == 0 {
            return;
        }
        let bits = bits as *mut u32;
        let width = WIDTH as usize;
        let height = HEIGHT as usize;

        // 1. 背景：圆角矩形内半透明，外部完全透明。
        for y in 0..height {
            for x in 0..width {
                let inside = rounded_inside(x as i32, y as i32, RADIUS);
                let pixel = if inside {
                    (BG_ALPHA << 24) | BG_RGB
                } else {
                    BG_RGB
                };
                *bits.add(y * width + x) = pixel;
            }
        }
        // 细边框
        for y in 2..height - 2 {
            for x in 2..width - 2 {
                let outer = rounded_inside(x as i32, y as i32, RADIUS);
                let inner = rounded_inside(x as i32 - 1, y as i32 - 1, (RADIUS - 1).max(1))
                    && x as i32 >= 2
                    && y as i32 >= 2;
                if outer && !inner {
                    *bits.add(y * width + x) = (150u32 << 24) | BORDER_RGB;
                }
            }
        }

        // 2. GDI 文字与进度条画在同一画布上。
        let mem = mem as HDC;
        let today = Local::now().date_naive();
        let snapshot = SNAPSHOT.with(|value| value.borrow().clone());
        let error = ERROR.with(|value| value.borrow().clone());

        text(mem, "totion", PADDING, PADDING, 0x00F0_F0F0, &FONT_TITLE);
        text(
            mem,
            &format!("{} {}", today.format("%Y-%m-%d"), weekday_label(today)),
            PADDING,
            PADDING + 34,
            0x00B0_A8A0,
            &FONT_BODY,
        );

        if let Some(snapshot) = snapshot {
            let todos = today_todos(&snapshot, today);
            let done = todos.iter().filter(|block| block.checked).count();
            let total = todos.len();
            let rate = if total == 0 {
                0.0
            } else {
                done as f64 / total as f64
            };
            text(
                mem,
                &format!("{done}/{total}  {:.0}%", rate * 100.0),
                PADDING,
                PADDING + 62,
                0x00F0_F0F0,
                &FONT_BODY,
            );
            bar(
                mem,
                PADDING + 100,
                PADDING + 68,
                WIDTH - PADDING * 2 - 100,
                10,
                rate,
            );

            // 最多显示 5 条：5 * 26px 不会压到底部的习惯/提示行。
            let mut y = PADDING + 96;
            if todos.is_empty() {
                text(mem, "今天没有待办", PADDING, y, 0x00B0_A8A0, &FONT_BODY);
            } else {
                for block in todos.iter().take(5) {
                    let mark = if block.checked { "✓" } else { "○" };
                    let color = if block.checked {
                        0x0050_B93F
                    } else {
                        0x00B0_A8A0
                    };
                    text(mem, mark, PADDING, y, color, &FONT_BODY);
                    let title = format!(
                        "{}{}",
                        block
                            .start_time
                            .as_deref()
                            .map(|time| format!("{time}  "))
                            .unwrap_or_default(),
                        block.text
                    );
                    text(
                        mem,
                        &truncate(&title, 30),
                        PADDING + 22,
                        y,
                        0x00F0_F0F0,
                        &FONT_BODY,
                    );
                    y += 26;
                }
                if todos.len() > 8 {
                    text(
                        mem,
                        &format!("还有 {} 条…", todos.len() - 5),
                        PADDING + 22,
                        y,
                        0x00B0_A8A0,
                        &FONT_SMALL,
                    );
                }
            }

            let (checked_habits, total_habits) = habits_summary(&snapshot, today);
            text(
                mem,
                &format!("习惯 {checked_habits}/{total_habits} 已打卡"),
                PADDING,
                HEIGHT - 52,
                0x00D8_D039,
                &FONT_SMALL,
            );
        } else {
            let message = error.unwrap_or_else(|| "正在连接…".to_string());
            text(
                mem,
                &truncate(&message, 30),
                PADDING,
                PADDING + 80,
                0x00B0_A8A0,
                &FONT_BODY,
            );
        }
        text(
            mem,
            "拖动移动 · 双击刷新 · 右键退出",
            PADDING,
            HEIGHT - 28,
            0x0090_8878,
            &FONT_SMALL,
        );

        // 3. 让 GDI 画过的像素变成不透明，背景保持半透明。
        for index in 0..width * height {
            let pixel = *bits.add(index);
            if pixel & 0x00FF_FFFF != BG_RGB {
                *bits.add(index) = pixel | 0xFF00_0000;
            }
        }

        // 4. 提交到分层窗口。
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
            cx: WIDTH,
            cy: HEIGHT,
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

    fn rounded_inside(x: i32, y: i32, radius: i32) -> bool {
        if x < 0 || y < 0 || x >= WIDTH || y >= HEIGHT {
            return false;
        }
        let cx_left = radius;
        let cy_top = radius;
        let cx_right = WIDTH - radius - 1;
        let cy_bottom = HEIGHT - radius - 1;
        let dx = if x < cx_left {
            cx_left - x
        } else if x > cx_right {
            x - cx_right
        } else {
            0
        };
        let dy = if y < cy_top {
            cy_top - y
        } else if y > cy_bottom {
            y - cy_bottom
        } else {
            0
        };
        dx * dx + dy * dy <= radius * radius
    }

    fn today_todos(snapshot: &Snapshot, today: NaiveDate) -> Vec<totion_core::Block> {
        let key = today.format("%Y-%m-%d").to_string();
        let mut todos: Vec<totion_core::Block> = snapshot
            .blocks
            .iter()
            .filter(|block| {
                !block.deleted
                    && block.due_date.as_deref() == Some(key.as_str())
                    && matches!(block.kind, BlockKind::Todo | BlockKind::CalendarEvent)
            })
            .cloned()
            .collect();
        todos.sort_by_key(|block| {
            (
                block
                    .start_time
                    .clone()
                    .unwrap_or_else(|| "99:99".to_string()),
                block.position,
            )
        });
        todos
    }

    fn habits_summary(snapshot: &Snapshot, today: NaiveDate) -> (usize, usize) {
        let mut total = 0;
        let mut checked = 0;
        for habit in snapshot
            .habits
            .iter()
            .filter(|habit| !habit.deleted && !habit.archived)
        {
            let stats = habit_stats(habit.frequency, &habit.id, &snapshot.checkins, today);
            if stats.checked_today || habit_due_on(habit.frequency, today, stats.weekly_done) {
                total += 1;
                if stats.checked_today {
                    checked += 1;
                }
            }
        }
        (checked, total)
    }

    unsafe fn text(
        hdc: HDC,
        value: &str,
        x: i32,
        y: i32,
        color: u32,
        font: &'static std::thread::LocalKey<Cell<isize>>,
    ) {
        let handle = font.with(|value| value.get());
        if handle == 0 {
            return;
        }
        SelectObject(hdc, handle as *mut c_void);
        SetBkMode(hdc, 1); // TRANSPARENT
        SetTextColor(hdc, color);
        let value = wide(value);
        TextOutW(hdc, x, y, value.as_ptr(), value.len() as i32 - 1);
    }

    unsafe fn bar(hdc: HDC, x: i32, y: i32, width: i32, height: i32, rate: f64) {
        let track = CreateSolidBrush(0x0030_2A24);
        let old = SelectObject(hdc, track);
        RoundRect(hdc, x, y, x + width, y + height, height, height);
        SelectObject(hdc, old);
        DeleteObject(track);

        let filled = (width as f64 * rate.clamp(0.0, 1.0)) as i32;
        if filled > 0 {
            let brush = CreateSolidBrush(0x00D8_D039);
            let old = SelectObject(hdc, brush);
            RoundRect(
                hdc,
                x,
                y,
                x + filled.max(height),
                y + height,
                height,
                height,
            );
            SelectObject(hdc, old);
            DeleteObject(brush);
        }
    }

    fn truncate(value: &str, limit: usize) -> String {
        let mut output: String = value.chars().take(limit).collect();
        if value.chars().count() > limit {
            output.push('…');
        }
        output
    }

    fn weekday_label(date: NaiveDate) -> &'static str {
        match date.weekday() {
            chrono::Weekday::Mon => "周一",
            chrono::Weekday::Tue => "周二",
            chrono::Weekday::Wed => "周三",
            chrono::Weekday::Thu => "周四",
            chrono::Weekday::Fri => "周五",
            chrono::Weekday::Sat => "周六",
            chrono::Weekday::Sun => "周日",
        }
    }
}
