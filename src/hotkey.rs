#[cfg(windows)]
mod platform {
    use std::{
        sync::mpsc::{self, Receiver},
        thread::{self, JoinHandle},
    };
    use windows::Win32::{
        Foundation::{LPARAM, WPARAM},
        System::Threading::GetCurrentThreadId,
        UI::{
            Input::KeyboardAndMouse::{
                MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey,
                VK_BACK,
            },
            WindowsAndMessaging::{
                GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_HOTKEY, WM_QUIT,
            },
        },
    };

    pub struct Hotkey {
        thread_id: u32,
        events: Receiver<()>,
        worker: Option<JoinHandle<()>>,
    }
    impl Hotkey {
        pub fn register() -> Result<Self, String> {
            let (ready_tx, ready_rx) = mpsc::channel();
            let (events_tx, events) = mpsc::channel();
            let worker = thread::spawn(move || unsafe {
                // Create the thread's message queue before publishing its ID.
                let mut message = MSG::default();
                let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
                let thread_id = GetCurrentThreadId();
                if RegisterHotKey(
                    None,
                    1,
                    MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_NOREPEAT,
                    VK_BACK.0 as u32,
                )
                .is_err()
                {
                    let _ = ready_tx.send(Err("Ctrl+Alt+Shift+Backspace is unavailable. Use the tray icon to open settings.".to_string()));
                    return;
                }
                let _ = ready_tx.send(Ok(thread_id));
                while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                    if message.message == WM_HOTKEY && message.wParam.0 == 1 {
                        let _ = events_tx.send(());
                    }
                }
                let _ = UnregisterHotKey(None, 1);
            });
            match ready_rx.recv().map_err(|e| e.to_string())? {
                Ok(thread_id) => Ok(Self {
                    thread_id,
                    events,
                    worker: Some(worker),
                }),
                Err(error) => {
                    let _ = worker.join();
                    Err(error)
                }
            }
        }
        pub fn pending_toggle(&self) -> bool {
            self.events.try_iter().count() % 2 != 0
        }
    }
    impl Drop for Hotkey {
        fn drop(&mut self) {
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }
}
#[cfg(windows)]
pub use platform::Hotkey;
#[cfg(not(windows))]
pub struct Hotkey;
#[cfg(not(windows))]
impl Hotkey {
    pub fn register() -> Result<Self, String> {
        Err("Use the tray icon to open settings on this platform.".into())
    }
    pub fn pending_toggle(&self) -> bool {
        false
    }
}
