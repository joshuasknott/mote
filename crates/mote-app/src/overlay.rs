//! Transparent overlay: a 256x256 layered window that follows Mote.
//!
//! - `WS_EX_LAYERED` + `UpdateLayeredWindow` for per-pixel alpha;
//! - `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` so it never steals focus and
//!   never appears in the taskbar / Alt-Tab;
//! - click-through everywhere except on the creature itself, implemented via
//!   `WM_NCHITTEST` (the app answers `HTTRANSPARENT` outside Mote's body);
//! - topmost, but hidden entirely while a fullscreen app is active (if the
//!   user asked for that) so games are never covered.

use mote_render::SPRITE_PX;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, AC_SRC_ALPHA,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP, HDC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, SetWindowPos, ShowWindow, UpdateLayeredWindow, HWND_TOPMOST, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, ULW_ALPHA, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_POPUP,
};

pub const OVERLAY_PX: i32 = SPRITE_PX as i32;
/// Window class name; must match the class registered in main.rs.
#[allow(dead_code)]
pub const CLASS_NAME: &str = "MoteOverlay";

pub struct Overlay {
    pub hwnd: HWND,
    mem_dc: HDC,
    dib: HBITMAP,
    /// Top-down 32-bit BGRA, premultiplied, OVERLAY_PX^2 pixels.
    bits: *mut u8,
    present_failed: bool,
}

impl Overlay {
    /// Create the window (hidden). `create_param` is passed through to
    /// `WM_NCCREATE` (the app stores its `*mut App` there).
    pub fn create(
        instance: HINSTANCE,
        create_param: *mut std::ffi::c_void,
    ) -> windows::core::Result<Self> {
        unsafe {
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("MoteOverlay"),
                w!("Mote"),
                WS_POPUP,
                -32000,
                -32000,
                OVERLAY_PX,
                OVERLAY_PX,
                None,
                None,
                Some(instance),
                Some(create_param),
            )?;

            // 32-bit top-down DIB section as the layered-window source.
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: OVERLAY_PX,
                biHeight: -OVERLAY_PX,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0)?;
            let mem_dc = CreateCompatibleDC(None);
            if mem_dc.is_invalid() {
                log::error!("CreateCompatibleDC failed; overlay will be blank");
            }
            let prev = SelectObject(mem_dc, dib.into());
            if prev.is_invalid() {
                log::error!("SelectObject(DIB) failed; overlay will be blank");
            }

            Ok(Self {
                hwnd,
                mem_dc,
                dib,
                bits: bits as *mut u8,
                present_failed: false,
            })
        }
    }

    /// Upload an RGBA premultiplied frame (converting to BGRA in place).
    pub fn set_pixels(&mut self, rgba: &[u8]) {
        debug_assert_eq!(rgba.len(), (OVERLAY_PX * OVERLAY_PX * 4) as usize);
        unsafe {
            let dst =
                std::slice::from_raw_parts_mut(self.bits, (OVERLAY_PX * OVERLAY_PX * 4) as usize);
            // RGBA -> BGRA swap.
            for (d, s) in dst.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
                d[0] = s[2];
                d[1] = s[1];
                d[2] = s[0];
                d[3] = s[3];
            }
        }
        // Diagnostic readback: verify the DIB actually holds what we wrote
        // (once). Catches stale/wrong `bits` pointers.
        static CHECKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !CHECKED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            let opaque_src = rgba.chunks_exact(4).filter(|p| p[3] > 128).count();
            let opaque_dib = unsafe {
                std::slice::from_raw_parts(self.bits, (OVERLAY_PX * OVERLAY_PX * 4) as usize)
                    .chunks_exact(4)
                    .filter(|p| p[3] > 128)
                    .count()
            };
            log::info!("overlay readback: src_opaque={opaque_src} dib_opaque={opaque_dib}");
        }
    }

    /// Blit the current bitmap at screen position (x, y). This is the
    /// single authority for the window's position — nothing else moves it.
    pub fn present(&mut self, x: i32, y: i32) {
        unsafe {
            let pt = POINT { x, y };
            let size = SIZE {
                cx: OVERLAY_PX,
                cy: OVERLAY_PX,
            };
            let src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: 0,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            if let Err(e) = UpdateLayeredWindow(
                self.hwnd,
                None,
                Some(&pt),
                Some(&size),
                Some(self.mem_dc),
                Some(&src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            ) {
                // Log once: per-frame spam would drown the log.
                static WARNED: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    log::warn!("UpdateLayeredWindow failed: {e:?}");
                }
                self.present_failed = true;
            } else {
                self.present_failed = false;
            }
        }
    }

    /// Show (topmost, without activating or moving) or hide the overlay.
    pub fn set_visible(&self, visible: bool) {
        unsafe {
            if visible {
                let _ = SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            } else {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
        }
    }

    pub fn present_failed(&self) -> bool {
        self.present_failed
    }

    #[cfg(test)]
    pub fn dummy(hwnd: HWND) -> Self {
        Self {
            hwnd,
            mem_dc: HDC::default(),
            dib: HBITMAP::default(),
            bits: std::ptr::null_mut(),
            present_failed: false,
        }
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        unsafe {
            if !self.hwnd.0.is_null() {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(self.hwnd);
            }
            if !self.dib.0.is_null() {
                let _ = DeleteObject(self.dib.into());
            }
            if !self.mem_dc.0.is_null() {
                let _ = DeleteDC(self.mem_dc);
            }
        }
    }
}

// SAFETY: only the UI thread touches the overlay.
unsafe impl Send for Overlay {}
