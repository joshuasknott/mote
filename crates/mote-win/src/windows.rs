//! Top-level window enumeration: the ledges Mote can stand on.
//!
//! Filters aggressively so helper/invisible windows never become surfaces:
//! visible, non-minimised, non-cloaked (UWP/Store suspension), real area,
//! on-screen, and not part of the shell chrome we model separately
//! (taskbar, wallpaper, the Mote overlay itself).

use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowTextLengthW,
    GetWindowTextW, IsIconic, IsWindowVisible,
};

/// Class names that must never become surfaces.
const EXCLUDED_CLASSES: &[&str] = &[
    "Shell_TrayWnd", // taskbar (modelled in taskbar.rs)
    "Shell_SecondaryTrayWnd",
    "Progman",                    // desktop wallpaper root
    "WorkerW",                    // desktop wallpaper worker
    "MoteOverlay",                // our own overlay window
    "Windows.UI.Core.CoreWindow", // start menu / search host shell bits — case by case
];

#[derive(Debug, Clone)]
pub struct TopLevelWindow {
    pub hwnd: HWND,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub title_len: usize,
    pub class_name: String,
}

impl TopLevelWindow {
    pub fn top(&self) -> i32 {
        self.y
    }
    pub fn left(&self) -> i32 {
        self.x
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    /// Stable id for [`mote_core::Support`]: the raw HWND value.
    pub fn support_id(&self) -> u64 {
        self.hwnd.0 as u64
    }
}

struct Ctx {
    out: Vec<TopLevelWindow>,
    virtual_rect: (i32, i32, i32, i32),
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut Ctx);
    if let Some(w) = describe_window(hwnd, ctx.virtual_rect) {
        ctx.out.push(w);
    }
    BOOL(1)
}

fn describe_window(hwnd: HWND, vr: (i32, i32, i32, i32)) -> Option<TopLevelWindow> {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return None;
        }
        if IsIconic(hwnd).as_bool() {
            return None; // minimised
        }
        // Cloaked (UWP suspended / virtual-desktop-hidden) windows.
        let mut cloaked = 0u32;
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut std::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        )
        .is_ok()
            && cloaked != 0
        {
            return None;
        }
        let mut rc = RECT::default();
        if GetWindowRect(hwnd, &mut rc).is_err() {
            return None;
        }
        let (x, y, w, h) = (rc.left, rc.top, rc.right - rc.left, rc.bottom - rc.top);
        if w < 80 || h < 40 {
            return None; // tooltips, shadows, helper windows
        }
        // Must intersect the virtual screen.
        let (vx, vy, vw, vh) = vr;
        if x + w <= vx || x >= vx + vw || y + h <= vy || y >= vy + vh {
            return None;
        }
        // Class-name exclusion.
        let mut cls = [0u16; 256];
        let n = GetClassNameW(hwnd, &mut cls);
        let class_name = if n > 0 {
            String::from_utf16_lossy(&cls[..n as usize])
        } else {
            String::new()
        };
        if EXCLUDED_CLASSES.iter().any(|e| class_name == *e) {
            return None;
        }
        // Skip windows with neither title nor meaningful size. Many
        // helper windows (IME, shell helpers) have empty titles; genuine
        // app windows almost always have one. Keep large untitled windows
        // (games, borderless apps) regardless.
        let title_len = GetWindowTextLengthW(hwnd).max(0) as usize;
        if title_len == 0 && (w < 300 || h < 200) {
            // Peek at the actual text only when needed (cheap enough).
            let mut buf = [0u16; 2];
            let _ = GetWindowTextW(hwnd, &mut buf);
            return None;
        }
        Some(TopLevelWindow {
            hwnd,
            x,
            y,
            w,
            h,
            title_len,
            class_name,
        })
    }
}

/// Enumerate candidate surface windows. Sorted: larger windows first so the
/// world builder prefers meaningful ledges when overlaps occur.
pub fn enumerate_windows() -> Vec<TopLevelWindow> {
    let vr = crate::monitors::virtual_screen_rect();
    let mut ctx = Ctx {
        out: Vec::new(),
        virtual_rect: (vr.x, vr.y, vr.w, vr.h),
    };
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut Ctx as isize));
    }
    ctx.out.sort_by_key(|w| -(w.w as i64 * w.h as i64));
    ctx.out
}

/// True when the foreground window covers a whole monitor (game / video).
/// The app uses this for "pause while fullscreen" behaviour.
pub fn foreground_is_fullscreen() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return false;
        }
        let mut rc = RECT::default();
        if GetWindowRect(fg, &mut rc).is_err() {
            return false;
        }
        crate::monitors::query_monitors().iter().any(|m| {
            rc.left <= m.x && rc.top <= m.y && rc.right >= m.x + m.w && rc.bottom >= m.y + m.h
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumeration_does_not_crash_and_excludes_shell() {
        let ws = enumerate_windows();
        for w in &ws {
            assert!(
                !EXCLUDED_CLASSES.contains(&w.class_name.as_str()),
                "shell window leaked: {}",
                w.class_name
            );
            assert!(w.w >= 80 && w.h >= 40);
        }
    }

    #[test]
    fn support_ids_unique() {
        let ws = enumerate_windows();
        let mut ids: Vec<u64> = ws.iter().map(|w| w.support_id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ws.len());
    }
}
