#[cfg(windows)]
mod platform {
    use std::cell::Cell;
    use windows::{
        Win32::{
            Foundation::{
                CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, LRESULT,
                POINT, WPARAM,
            },
            System::Threading::CreateMutexW,
            UI::{Shell::*, WindowsAndMessaging::*},
        },
        core::w,
    };
    const TRAY_MESSAGE: u32 = WM_APP + 65;
    const SUBCLASS_ID: usize = 0x4d52;
    const CLOSE_ID: isize = 0x5001;
    thread_local! {
        static COMMANDS: Cell<u32> = const { Cell::new(0) };
        static MENU: Cell<HMENU> = const { Cell::new(HMENU(std::ptr::null_mut())) };
    }
    pub struct Instance(HANDLE);
    impl Instance {
        pub fn acquire() -> Result<Self, String> {
            unsafe {
                let handle = CreateMutexW(None, false, w!("Local\\Minuter-rungui-experiment"))
                    .map_err(|e| e.to_string())?;
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    let _ = CloseHandle(handle);
                    return Err("The experimental Minuter is already running. Use its tray icon or Ctrl+Alt+Shift+Backspace.".into());
                }
                Ok(Self(handle))
            }
        }
    }
    impl Drop for Instance {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    pub struct Tray {
        data: NOTIFYICONDATAW,
        menu: HMENU,
        attached: Cell<bool>,
    }
    impl Tray {
        pub fn new(window: usize, close_button: usize) -> Result<Self, String> {
            unsafe {
                let hwnd = HWND(window as *mut _);
                let close = HWND(close_button as *mut _);
                // The default button is a direct child of the native window, for IsDialogMessage.
                SetWindowLongPtrW(close, GWLP_ID, CLOSE_ID);
                SendMessageW(close, 0x00f4, Some(WPARAM(1)), Some(LPARAM(1))); // BM_SETSTYLE, BS_DEFPUSHBUTTON
                if !SetWindowSubclass(hwnd, Some(window_proc), SUBCLASS_ID, 0).as_bool() {
                    return Err("Could not install native window integration".into());
                }
                let menu = CreatePopupMenu().map_err(|e| e.to_string())?;
                let menu_result = (|| -> windows::core::Result<()> {
                    AppendMenuW(menu, MF_STRING, 0x6001, w!("Show settings"))?;
                    AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null())?;
                    AppendMenuW(menu, MF_STRING, 0x6002, w!("Disable all"))?;
                    AppendMenuW(menu, MF_STRING, 0x6003, w!("Quit Minuter"))?;
                    Ok(())
                })();
                if let Err(e) = menu_result {
                    let _ = DestroyMenu(menu);
                    let _ = RemoveWindowSubclass(hwnd, Some(window_proc), SUBCLASS_ID);
                    return Err(e.to_string());
                }
                MENU.with(|m| m.set(menu));
                let mut data = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: hwnd,
                    uID: 1,
                    uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                    uCallbackMessage: TRAY_MESSAGE,
                    hIcon: LoadIconW(None, IDI_INFORMATION).map_err(|e| e.to_string())?,
                    ..Default::default()
                };
                for (slot, code) in data
                    .szTip
                    .iter_mut()
                    .zip("Minuter — rungui experiment".encode_utf16())
                {
                    *slot = code;
                }
                if !Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
                    let _ = DestroyMenu(menu);
                    let _ = RemoveWindowSubclass(hwnd, Some(window_proc), SUBCLASS_ID);
                    return Err("Could not create the tray icon".into());
                }
                Ok(Self {
                    data,
                    menu,
                    attached: Cell::new(true),
                })
            }
        }
        pub fn commands(&self) -> u32 {
            COMMANDS.with(|c| c.replace(0))
        }
        pub fn detach(&self) {
            if !self.attached.replace(false) {
                return;
            }
            unsafe {
                let _ = Shell_NotifyIconW(NIM_DELETE, &self.data);
                let _ = RemoveWindowSubclass(self.data.hWnd, Some(window_proc), SUBCLASS_ID);
                let _ = DestroyMenu(self.menu);
            }
            MENU.with(|m| m.set(HMENU(std::ptr::null_mut())));
        }
    }
    impl Drop for Tray {
        fn drop(&mut self) {
            self.detach();
        }
    }
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wp: WPARAM,
        lp: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        // No Rust application callback runs inside the native subclass or menu loop.
        if msg == 0x400 {
            return LRESULT(0x534b0000 | CLOSE_ID);
        } // DM_GETDEFID
        if msg == TRAY_MESSAGE {
            let command = match lp.0 as u32 {
                WM_LBUTTONDBLCLK => 1,
                WM_RBUTTONUP => unsafe {
                    let mut point = POINT::default();
                    let _ = GetCursorPos(&mut point);
                    let _ = SetForegroundWindow(hwnd);
                    let selected = MENU
                        .with(|m| {
                            TrackPopupMenu(
                                m.get(),
                                TPM_RETURNCMD | TPM_NONOTIFY,
                                point.x,
                                point.y,
                                None,
                                hwnd,
                                None,
                            )
                        })
                        .0 as usize;
                    let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
                    match selected {
                        0x6001 => 1,
                        0x6002 => 2,
                        0x6003 => 4,
                        _ => 0,
                    }
                },
                _ => 0,
            };
            COMMANDS.with(|c| c.set(c.get() | command));
            return LRESULT(0);
        }
        unsafe { DefSubclassProc(hwnd, msg, wp, lp) }
    }
}
#[cfg(windows)]
pub use platform::{Instance, Tray};
#[cfg(not(windows))]
pub struct Instance;
#[cfg(not(windows))]
impl Instance {
    pub fn acquire() -> Result<Self, String> {
        Ok(Self)
    }
}
#[cfg(not(windows))]
pub struct Tray;
#[cfg(not(windows))]
impl Tray {
    pub fn new(_window: usize, _close: usize) -> Result<Self, String> {
        Err("Experimental tray support is currently Windows-only.".into())
    }
    pub fn commands(&self) -> u32 {
        0
    }
    pub fn detach(&self) {}
}
