//! Windows 桌面集成：把窗口挂到桌面上、抓取窗口背后的壁纸做毛玻璃底。

#![cfg(windows)]

use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
    SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetClassNameW, GetWindowRect, IsWindowVisible,
    SendMessageTimeoutW, SetParent, SetWindowLongW, SetWindowPos, ShowWindow, GWL_EXSTYLE,
    HWND_BOTTOM, HWND_TOP, SMTO_NORMAL, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOW,
    WS_EX_TOOLWINDOW,
};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 桌面的主屏尺寸（物理像素）。
pub fn screen_size() -> (i32, i32) {
    unsafe {
        (
            windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(0),
            windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(1),
        )
    }
}

/// 抓取屏幕上的一块区域（物理像素），返回 BGRA 像素，行优先、自上而下。
pub fn capture(x: i32, y: i32, w: i32, h: i32) -> Option<Vec<u8>> {
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        if screen.is_null() {
            return None;
        }
        let mem = CreateCompatibleDC(screen);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(mem, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if bitmap.is_null() {
            DeleteDC(mem);
            ReleaseDC(std::ptr::null_mut(), screen);
            return None;
        }
        let old = SelectObject(mem, bitmap);
        let ok = BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY);
        let data = if ok != 0 && !bits.is_null() {
            let len = (w * h * 4) as usize;
            Some(std::slice::from_raw_parts(bits as *const u8, len).to_vec())
        } else {
            None
        };
        SelectObject(mem, old);
        DeleteObject(bitmap);
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);
        data
    }
}

/// 把窗口贴到桌面层（壁纸之上、普通窗口之下），并避免抢焦点。
pub fn anchor_bottom(hwnd: isize) {
    unsafe {
        let window = hwnd as HWND;
        let current = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW(window, GWL_EXSTYLE) as u32;
        // 去掉 WS_EX_APPWINDOW，否则任务栏会留一个按钮。
        let ex = (current & !0x0004_0000) | WS_EX_TOOLWINDOW;
        SetWindowLongW(window, GWL_EXSTYLE, ex as i32);
        SetWindowPos(
            window,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

pub fn attach_to_desktop(hwnd: isize) -> bool {
    unsafe {
        let window = hwnd as HWND;
        if window.is_null() {
            return false;
        }
        // 注意：这里必须是「叠加」而不是覆盖，覆盖会把 winit 为透明窗口设的扩展样式冲掉。
        let current = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW(window, GWL_EXSTYLE) as u32;
        let ex = current | WS_EX_TOOLWINDOW;
        SetWindowLongW(window, GWL_EXSTYLE, ex as i32);

        // 让 Progman 生成 WorkerW，再把窗口挂到承载壁纸的那个 WorkerW 上。
        let progman = FindWindowW(wide("Progman").as_ptr(), std::ptr::null());
        if !progman.is_null() {
            let mut result: usize = 0;
            SendMessageTimeoutW(progman, 0x052C, 0, 0, SMTO_NORMAL, 1000, &mut result);
        }
        // 跨进程 SetParent 要求 DPI 感知上下文一致，否则会失败（返回 NULL）。
        let previous = windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(
            windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
        if let Some(worker) = find_wallpaper_worker() {
            SetParent(window, worker);
            SetWindowPos(
                window,
                HWND_BOTTOM,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            log_line(&format!(
                "parent=worker {}",
                class_name(worker as isize)
            ));
            windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(previous);
            return true;
        }
        // Win11 上常常没有独立的壁纸 WorkerW，直接挂到 Progman。
        if !progman.is_null() {
            SetParent(window, progman);
            // 放到 Progman 子窗口的顶层：壁纸之上，像贴在墙上。
            SetWindowPos(
                window,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            log_line(&format!(
                "parent=progman {}",
                class_name(progman as isize)
            ));
            windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(previous);
            return true;
        }
        windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(previous);
        // 退路：留在桌面底层。
        SetWindowPos(
            window,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        false
    }
}

unsafe fn find_wallpaper_worker() -> Option<HWND> {
    struct State {
        found: Option<HWND>,
    }
    unsafe extern "system" fn callback(top: HWND, param: LPARAM) -> BOOL {
        let state = &mut *(param as *mut State);
        if IsWindowVisible(top) == 0 {
            return 1;
        }
        let shell = FindWindowExW(top, std::ptr::null_mut(), wide("SHELLDLL_DefView").as_ptr(), std::ptr::null());
        if shell.is_null() {
            return 1;
        }
        let worker = FindWindowExW(
            std::ptr::null_mut(),
            top,
            wide("WorkerW").as_ptr(),
            std::ptr::null(),
        );
        if !worker.is_null() {
            state.found = Some(worker);
            return 0;
        }
        1
    }
    let mut state = State { found: None };
    EnumWindows(Some(callback), &mut state as *mut State as LPARAM);
    state.found
}

pub fn set_visible(hwnd: isize, visible: bool) {
    unsafe {
        ShowWindow(hwnd as HWND, if visible { SW_SHOW } else { SW_HIDE });
    }
}

/// 窗口矩形（物理像素）。
pub fn window_rect(hwnd: isize) -> Option<(i32, i32, i32, i32)> {
    unsafe {
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        if GetWindowRect(hwnd as HWND, &mut rect) == 0 {
            return None;
        }
        Some((
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
        ))
    }
}

/// 找到本进程真正可见的主窗口（raw handle 有时拿到的是隐藏的内部窗口）。
pub fn main_window() -> Option<isize> {
    use windows_sys::Win32::Foundation::RECT as R;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowThreadProcessId, IsWindowVisible as Visible,
    };
    struct State {
        pid: u32,
        best: Option<(isize, i64)>,
    }
    unsafe extern "system" fn callback(top: HWND, param: LPARAM) -> BOOL {
        let state = &mut *(param as *mut State);
        let mut pid = 0u32;
        GetWindowThreadProcessId(top, &mut pid);
        if pid != state.pid || Visible(top) == 0 {
            return 1;
        }
        let mut rect: R = std::mem::zeroed();
        if GetWindowRect(top, &mut rect) == 0 {
            return 1;
        }
        let area = (rect.right - rect.left) as i64 * (rect.bottom - rect.top) as i64;
        if area > 100_000 && state.best.map(|(_, a)| area > a).unwrap_or(true) {
            state.best = Some((top as isize, area));
        }
        1
    }
    let mut state = State {
        pid: std::process::id(),
        best: None,
    };
    unsafe {
        EnumWindows(Some(callback), &mut state as *mut State as LPARAM);
    }
    state.best.map(|(hwnd, _)| hwnd)
}

/// eframe 的窗口句柄转成 Win32 句柄。
pub fn hwnd_of(frame: &eframe::Frame) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = frame.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win) => Some(win.hwnd.get()),
        _ => None,
    }
}

/// 调试用：窗口类名。
pub fn class_name(hwnd: isize) -> String {
    let mut buffer = [0u16; 128];
    unsafe {
        let len = GetClassNameW(hwnd as HWND, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..len.max(0) as usize])
    }
}

/// 把窗口精确放到屏幕右半（物理像素），不受 DPI 缩放影响。
pub fn place_right_half(hwnd: isize) {
    let (width, height) = screen_size();
    unsafe {
        SetWindowPos(
            hwnd as HWND,
            HWND_TOP,
            width / 2,
            0,
            width - width / 2,
            height,
            0,
        );
    }
}

/// 临时诊断：把一行写进临时目录的日志。
pub fn log_line(text: &str) {
    use std::io::Write;
    if std::env::var_os("GLASSWING_DEBUG").is_none() {
        return;
    }
    let path = std::env::temp_dir().join("glasswing.log");
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{text}");
    }
}

/// 当前桌面壁纸的绝对路径（读注册表）。
pub fn wallpaper_path() -> Option<String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETDESKWALLPAPER};
    let mut buffer = [0u16; 1024];
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buffer.len() as u32,
            buffer.as_mut_ptr() as *mut std::ffi::c_void,
            0,
        )
    };
    if ok == 0 {
        return None;
    }
    let len = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
    let text = String::from_utf16_lossy(&buffer[..len]);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// 诊断：父窗口、可见性、矩形、扩展样式。
pub fn describe(hwnd: isize) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetParent, GetWindowLongW, IsWindowVisible};
    unsafe {
        let window = hwnd as HWND;
        let parent = GetParent(window);
        let ex = GetWindowLongW(window, GWL_EXSTYLE) as u32;
        let rect = window_rect(hwnd).map(|(x, y, w, h)| format!("{x},{y} {w}x{h}"));
        format!(
            "visible={} parent={:?} rect={:?} exstyle=0x{ex:08X} layered={} toolwindow={}",
            IsWindowVisible(window),
            if parent.is_null() {
                "none".to_string()
            } else {
                class_name(parent as isize)
            },
            rect,
            ex & 0x80000 != 0,
            ex & 0x80 != 0
        )
    }
}

/// 诊断：列出本进程所有顶层窗口。
pub fn list_own_windows() -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowThreadProcessId, IsWindowVisible as Visible,
    };
    struct State {
        pid: u32,
        out: Vec<String>,
    }
    unsafe extern "system" fn callback(top: HWND, param: LPARAM) -> BOOL {
        let state = &mut *(param as *mut State);
        let mut pid = 0u32;
        GetWindowThreadProcessId(top, &mut pid);
        if pid == state.pid {
            let rect = window_rect(top as isize)
                .map(|(x, y, w, h)| format!("{x},{y},{w}x{h}"))
                .unwrap_or_default();
            state.out.push(format!(
                "{top:?} {} visible={} rect={rect}",
                class_name(top as isize),
                Visible(top)
            ));
        }
        1
    }
    let mut state = State {
        pid: std::process::id(),
        out: Vec::new(),
    };
    unsafe {
        EnumWindows(Some(callback), &mut state as *mut State as LPARAM);
    }
    state.out.join(" | ")
}

/// 去掉任务栏按钮（WS_EX_APPWINDOW）。
pub fn clear_appwindow(hwnd: isize) {
    unsafe {
        let window = hwnd as HWND;
        let current = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW(window, GWL_EXSTYLE) as u32;
        let ex = (current & !0x0004_0000) | WS_EX_TOOLWINDOW;
        SetWindowLongW(window, GWL_EXSTYLE, ex as i32);
    }
}

/// 工作区（不含任务栏）在屏幕坐标系里的位置与大小。
pub fn work_area() -> (i32, i32, i32, i32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA};
    unsafe {
        let mut rect: RECT = std::mem::zeroed();
        let ok = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            &mut rect as *mut RECT as *mut std::ffi::c_void,
            0,
        );
        if ok == 0 {
            let (w, h) = screen_size();
            return (0, 0, w, h);
        }
        (
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
        )
    }
}

/// 把窗口放到工作区右半，避开任务栏，保证上下留白对称。
pub fn place_right_half_workarea(hwnd: isize) {
    let (ax, ay, aw, ah) = work_area();
    unsafe {
        // SWP_NOZORDER：只改位置和大小，绝不动层级，避免把面板顶到别的窗口上面。
        SetWindowPos(
            hwnd as HWND,
            HWND_BOTTOM,
            ax + aw / 2,
            ay,
            aw - aw / 2,
            ah,
            0x0004,
        );
    }
}

/// 把窗口钉在桌面之上、所有普通窗口之下：
/// 先把自己降到底，再把桌面（Progman）压到自己下面。
pub fn anchor_above_desktop(hwnd: isize) {
    unsafe {
        let window = hwnd as HWND;
        let progman = FindWindowW(wide("Progman").as_ptr(), std::ptr::null());
        SetWindowPos(
            window,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        if !progman.is_null() && progman != window {
            SetWindowPos(
                progman,
                HWND_BOTTOM,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}
