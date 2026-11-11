//! Native video surface.
//!
//! libVLC needs a real OS window to render into. We create a borderless Win32
//! child window inside the Tauri main window and keep it positioned over the
//! `.video-area` element of the Leptos UI (the UI reports that element's bounds).

use std::ffi::c_void;

#[cfg(windows)]
mod imp {
    use super::c_void;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Gdi::{GetStockObject, BLACK_BRUSH, HBRUSH};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassW, SetWindowPos, ShowWindowAsync,
        CS_HREDRAW, CS_VREDRAW, HWND_TOP, IDC_ARROW, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SW_HIDE,
        SW_SHOWNA, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    };

    pub struct VideoSurface {
        hwnd: HWND,
    }

    // The HWND is just a handle; the calls we make on it are safe cross-thread
    // (async window-pos / show requests are posted to the owning UI thread).
    unsafe impl Send for VideoSurface {}
    unsafe impl Sync for VideoSurface {}

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    impl VideoSurface {
        /// Must be called on the UI (main) thread so the window gets a message loop.
        pub fn create(parent: *mut c_void) -> Result<Self, String> {
            unsafe {
                let class = wide("DotaraVideoSurface");
                let hinstance = GetModuleHandleW(null());
                let wc = WNDCLASSW {
                    style: CS_HREDRAW | CS_VREDRAW,
                    lpfnWndProc: Some(DefWindowProcW),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: hinstance,
                    hIcon: null_mut(),
                    hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                    hbrBackground: GetStockObject(BLACK_BRUSH) as HBRUSH,
                    lpszMenuName: null(),
                    lpszClassName: class.as_ptr(),
                };
                // Fails harmlessly if the class is already registered.
                RegisterClassW(&wc);

                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    null(),
                    WS_CHILD | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                    0,
                    0,
                    0,
                    0,
                    parent as HWND,
                    null_mut(),
                    hinstance,
                    null(),
                );
                if hwnd.is_null() {
                    return Err("CreateWindowExW failed for the video surface".into());
                }
                Ok(Self { hwnd })
            }
        }

        pub fn raw(&self) -> *mut c_void {
            self.hwnd as *mut c_void
        }

        /// Position in physical pixels, relative to the parent's client area.
        pub fn set_bounds(&self, x: i32, y: i32, w: i32, h: i32) {
            unsafe {
                SetWindowPos(self.hwnd, HWND_TOP, x, y, w.max(0), h.max(0), SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
            }
        }

        pub fn set_visible(&self, visible: bool) {
            unsafe {
                ShowWindowAsync(self.hwnd, if visible { SW_SHOWNA } else { SW_HIDE });
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::c_void;

    /// Placeholder: video embedding is only implemented for Windows so far.
    pub struct VideoSurface;

    impl VideoSurface {
        pub fn create(_parent: *mut c_void) -> Result<Self, String> {
            Err("video embedding is only implemented on Windows for now".into())
        }
        pub fn raw(&self) -> *mut c_void {
            std::ptr::null_mut()
        }
        pub fn set_bounds(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}
        pub fn set_visible(&self, _visible: bool) {}
    }
}

pub use imp::VideoSurface;
