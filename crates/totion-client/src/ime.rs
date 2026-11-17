//! 尽量抑制 Windows 控制台里「按 j/k 却弹出输入法」的问题。
//!
//! 思路：非编辑状态下把控制台窗口的输入法上下文解绑，进入编辑/连接界面时再挂回去。
//! 只对传统控制台（conhost）有效；Windows Terminal 由终端自己管理输入法，
//! 这种情况需要用户把输入法切到英文模式。

#[cfg(windows)]
pub fn set_enabled(enabled: bool) {
    use std::sync::atomic::{AtomicIsize, Ordering};

    use windows_sys::Win32::System::Console::GetConsoleWindow;
    use windows_sys::Win32::UI::Input::Ime::ImmAssociateContext;

    static SAVED: AtomicIsize = AtomicIsize::new(0);

    unsafe {
        let hwnd = GetConsoleWindow();
        if hwnd.is_null() {
            return;
        }
        if enabled {
            let saved = SAVED.swap(0, Ordering::SeqCst);
            if saved != 0 {
                ImmAssociateContext(hwnd, saved as _);
            }
        } else {
            let previous = ImmAssociateContext(hwnd, std::ptr::null_mut());
            if !previous.is_null() {
                SAVED.store(previous as isize, Ordering::SeqCst);
            }
        }
    }
}

#[cfg(not(windows))]
pub fn set_enabled(_enabled: bool) {}
