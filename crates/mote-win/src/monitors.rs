//! Monitor enumeration: bounds, DPI and the virtual-screen rect.
//!
//! Uses `EnumDisplayMonitors` + `GetMonitorInfoW` + `GetDpiForMonitor` so
//! per-monitor DPI is honoured. All coordinates are virtual-screen pixels,
//! matching the coordinate space physics simulates in.

use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics;
use windows::Win32::UI::WindowsAndMessaging::{
    SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorInfo {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Effective DPI (96 = 100%).
    pub dpi: u32,
    pub primary: bool,
}

impl MonitorInfo {
    pub fn scale(&self) -> f32 {
        self.dpi as f32 / 96.0
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualScreen {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Virtual-screen rect via GetSystemMetrics (never fails in practice; falls
/// back to the primary monitor on error).
pub fn virtual_screen_rect() -> VirtualScreen {
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if w > 0 && h > 0 {
            VirtualScreen { x, y, w, h }
        } else {
            VirtualScreen {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
            }
        }
    }
}

/// MONITORINFOF_PRIMARY (no named constant in windows-rs 0.61).
const MONITORINFOF_PRIMARY: u32 = 1;

struct EnumCtx {
    monitors: Vec<MonitorInfo>,
    primary_handle: Option<HMONITOR>,
}

unsafe extern "system" fn enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut EnumCtx);
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
        let rc = info.rcMonitor;
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        // DPI failure is non-fatal: assume 96.
        let _ = GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        ctx.monitors.push(MonitorInfo {
            x: rc.left,
            y: rc.top,
            w: (rc.right - rc.left).max(1),
            h: (rc.bottom - rc.top).max(1),
            dpi: dpi_x.clamp(48, 384),
            primary: Some(hmonitor) == ctx.primary_handle
                || (info.dwFlags & MONITORINFOF_PRIMARY) != 0,
        });
    }
    BOOL(1)
}

/// Enumerate all display monitors. Returns at least a fallback entry.
pub fn query_monitors() -> Vec<MonitorInfo> {
    unsafe {
        let primary = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut ctx = EnumCtx {
            monitors: Vec::new(),
            primary_handle: Some(primary),
        };
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut ctx as *mut EnumCtx as isize),
        );
        if ctx.monitors.is_empty() {
            let v = virtual_screen_rect();
            ctx.monitors.push(MonitorInfo {
                x: v.x,
                y: v.y,
                w: v.w,
                h: v.h,
                dpi: 96,
                primary: true,
            });
        }
        // Deterministic order: primary first, then left-to-right.
        ctx.monitors.sort_by_key(|m| (!m.primary, m.x, m.y));
        ctx.monitors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_screen_sane() {
        let v = virtual_screen_rect();
        assert!(v.w >= 800 && v.h >= 600, "unexpected virtual screen {v:?}");
    }

    #[test]
    fn monitors_nonempty() {
        let ms = query_monitors();
        assert!(!ms.is_empty());
        assert!(ms.iter().any(|m| m.primary));
    }
}
