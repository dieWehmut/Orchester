//! Windows 11 requires HTMAXBUTTON on the real top-level HWND for Snap Layouts.
//! https://learn.microsoft.com/windows/apps/desktop/modernize/ui/apply-snap-layout-menu

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::{Dwm::DwmDefWindowProc, Gdi::ScreenToClient},
    UI::{
        HiDpi::GetDpiForWindow,
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            DefWindowProcW, GetClientRect, HTMAXBUTTON, WM_NCDESTROY, WM_NCHITTEST,
            WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE,
        },
    },
};

use crate::caption_geometry::is_maximize_target;

const SUBCLASS_ID: usize = 0x4f524348;

/// Attach on the window's UI thread. No pointers or heap-owned callback state are retained.
pub fn install(hwnd: HWND) -> std::io::Result<()> {
    // SAFETY: SetWindowSubclass validates the HWND and thread. Our callback has a static
    // lifetime, retains no borrowed data, and removes itself during WM_NCDESTROY.
    if unsafe { SetWindowSubclass(hwnd, Some(caption_proc), SUBCLASS_ID, 0) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

unsafe extern "system" fn caption_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    // SAFETY: Windows calls this on the owning UI thread with a live HWND. Stack
    // structures are initialized; packed coordinates are signed (negative monitors).
    unsafe {
        if message == WM_NCDESTROY {
            RemoveWindowSubclass(hwnd, Some(caption_proc), SUBCLASS_ID);
        }
        if message == WM_NCHITTEST {
            let mut point = POINT {
                x: (lparam as u16 as i16) as i32,
                y: ((lparam >> 16) as u16 as i16) as i32,
            };
            let mut bounds = RECT::default();
            if ScreenToClient(hwnd, &mut point) != 0
                && GetClientRect(hwnd, &mut bounds) != 0
                && is_maximize_target(bounds.right, point.x, point.y, GetDpiForWindow(hwnd))
            {
                return HTMAXBUTTON as LRESULT;
            }
        }
        // DWM owns the Windows 11 hover popup. Keep its non-client mouse routing
        // intact even though the caption glyph itself is drawn in the webview.
        if matches!(message, WM_NCMOUSEMOVE | WM_NCMOUSELEAVE) {
            let mut result = 0;
            if DwmDefWindowProc(hwnd, message, wparam, lparam, &mut result) != 0 {
                return result;
            }
        }
        if matches!(message, WM_NCLBUTTONDOWN | WM_NCLBUTTONUP) && wparam == HTMAXBUTTON as usize {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        // All other caption/resize/client behavior continues through Tauri/Wry.
        DefSubclassProc(hwnd, message, wparam, lparam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr::null_mut;
    use windows_sys::Win32::{
        Foundation::{POINT, RECT},
        Graphics::Gdi::ClientToScreen,
        UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
    };

    #[test]
    fn real_window_exposes_the_maximize_region_to_windows_hit_testing() {
        // SAFETY: a private, hidden HWND is created, used and destroyed on this test thread.
        unsafe {
            let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                100,
                100,
                1280,
                800,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            );
            assert!(!hwnd.is_null());
            install(hwnd).unwrap();
            let mut rect = RECT::default();
            assert_ne!(GetClientRect(hwnd, &mut rect), 0);
            let dpi = GetDpiForWindow(hwnd) as i32;
            let mut point = POINT {
                x: rect.right - (69 * dpi / 96),
                y: 16 * dpi / 96,
            };
            assert_ne!(ClientToScreen(hwnd, &mut point), 0);
            let packed = ((point.y as u32 & 0xffff) << 16) | (point.x as u32 & 0xffff);
            let hit = SendMessageW(hwnd, WM_NCHITTEST, 0, packed as isize);
            point.x += 46 * dpi / 96;
            let close_point = ((point.y as u32 & 0xffff) << 16) | (point.x as u32 & 0xffff);
            let close_hit = SendMessageW(hwnd, WM_NCHITTEST, 0, close_point as isize);
            DestroyWindow(hwnd);
            assert_eq!(hit, HTMAXBUTTON as isize);
            assert_ne!(close_hit, HTMAXBUTTON as isize);
        }
    }
}
