//! Windows taskbar integration: progress on the taskbar button and flashing when a job ends
//! while the window is in the background. No-op on other platforms.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskbarState {
    None,
    Indeterminate,
    /// 0.0 ..= 1.0
    Progress(f32),
    Error,
}

#[derive(Default)]
pub struct Taskbar {
    #[cfg(windows)]
    inner: Option<(windows::Win32::UI::Shell::ITaskbarList3, windows::Win32::Foundation::HWND)>,
    #[cfg(windows)]
    failed: bool,
    last: Option<(u8, u32)>,
}

#[cfg(windows)]
fn hwnd_of(frame: &eframe::Frame) -> Option<windows::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match frame.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(w) => Some(windows::Win32::Foundation::HWND(w.hwnd.get())),
        _ => None,
    }
}

impl Taskbar {
    pub fn set(&mut self, frame: &eframe::Frame, state: TaskbarState) {
        let key = match state {
            TaskbarState::None => (0, 0),
            TaskbarState::Indeterminate => (1, 0),
            TaskbarState::Progress(p) => (2, (p.clamp(0.0, 1.0) * 1000.0) as u32),
            TaskbarState::Error => (3, 0),
        };
        if self.last == Some(key) {
            return;
        }
        self.last = Some(key);

        #[cfg(windows)]
        {
            use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
            use windows::Win32::UI::Shell::{
                ITaskbarList3, TaskbarList, TBPF_ERROR, TBPF_INDETERMINATE, TBPF_NOPROGRESS, TBPF_NORMAL,
            };
            if self.inner.is_none() && !self.failed {
                // SAFETY: COM is initialised on the UI thread by winit (OLE drag & drop).
                let created = unsafe {
                    CoCreateInstance::<_, ITaskbarList3>(&TaskbarList, None, CLSCTX_INPROC_SERVER)
                        .and_then(|t| t.HrInit().map(|_| t))
                };
                match (created, hwnd_of(frame)) {
                    (Ok(t), Some(h)) => self.inner = Some((t, h)),
                    _ => self.failed = true,
                }
            }
            if let Some((tb, hwnd)) = &self.inner {
                // SAFETY: valid interface and window handle owned by this process.
                unsafe {
                    let _ = match state {
                        TaskbarState::None => tb.SetProgressState(*hwnd, TBPF_NOPROGRESS),
                        TaskbarState::Indeterminate => tb.SetProgressState(*hwnd, TBPF_INDETERMINATE),
                        TaskbarState::Progress(_) => tb
                            .SetProgressState(*hwnd, TBPF_NORMAL)
                            .and_then(|_| tb.SetProgressValue(*hwnd, key.1 as u64, 1000)),
                        TaskbarState::Error => tb
                            .SetProgressState(*hwnd, TBPF_ERROR)
                            .and_then(|_| tb.SetProgressValue(*hwnd, 1000, 1000)),
                    };
                }
            }
        }
        #[cfg(not(windows))]
        let _ = frame;
    }

    /// Flash the taskbar button until the window is focused.
    pub fn flash(&self, frame: &eframe::Frame) {
        #[cfg(windows)]
        if let Some(hwnd) = hwnd_of(frame) {
            use windows::Win32::UI::WindowsAndMessaging::{FlashWindowEx, FLASHWINFO, FLASHW_ALL, FLASHW_TIMERNOFG};
            let info = FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd,
                dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
                uCount: 5,
                dwTimeout: 0,
            };
            // SAFETY: plain struct + our own window handle.
            unsafe {
                let _ = FlashWindowEx(&info);
            }
        }
        #[cfg(not(windows))]
        let _ = frame;
    }
}
