//! Taskbar geometry: position, bounds, edge and auto-hide state.
//!
//! Uses `SHAppBarMessage(ABM_GETTASKBARPOS)` for the authoritative edge +
//! rect, and the `Shell_TrayWnd` window rect as a cross-check. Auto-hide is
//! read via `ABM_GETSTATE`. Anything that fails degrades to "bottom edge of
//! the primary monitor, not hidden".

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::Shell::{
    SHAppBarMessage, ABM_GETSTATE, ABM_GETTASKBARPOS, ABS_AUTOHIDE, APPBARDATA,
};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowRect};

use crate::monitors::MonitorInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarEdge {
    Bottom,
    Top,
    Left,
    Right,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskbarInfo {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub edge: TaskbarEdge,
    pub autohide: bool,
    /// Best-effort monitor index the taskbar sits on.
    pub monitor: u32,
}

impl TaskbarInfo {
    /// Y of the surface Mote stands on (top of a bottom taskbar).
    /// Returns None for side/top taskbars where "standing" is meaningless —
    /// the caller falls back to the top edge of the bar as a narrow ledge.
    pub fn stand_y(&self) -> Option<i32> {
        match self.edge {
            TaskbarEdge::Bottom => Some(self.y),
            _ => None,
        }
    }
}

/// Query current taskbar geometry. Never panics; never returns Err to the
/// caller — failures yield a bottom-edge default on the primary monitor.
pub fn query_taskbar(monitors: &[MonitorInfo]) -> TaskbarInfo {
    let fallback = fallback_taskbar(monitors);

    unsafe {
        let mut abd = APPBARDATA {
            cbSize: std::mem::size_of::<APPBARDATA>() as u32,
            ..Default::default()
        };
        let pos_ok = SHAppBarMessage(ABM_GETTASKBARPOS, &mut abd) != 0;
        let state = SHAppBarMessage(ABM_GETSTATE, &mut abd);
        let autohide = (state as u32 & ABS_AUTOHIDE) != 0;

        if pos_ok {
            let rc = abd.rc;
            let (mut x, mut y, mut w, mut h) = (
                rc.left,
                rc.top,
                (rc.right - rc.left).max(1),
                (rc.bottom - rc.top).max(1),
            );
            // Cross-check with the tray window rect; if the bar says one
            // thing and the window another, prefer the window rect.
            if let Ok(hwnd) = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) {
                let mut wr = RECT::default();
                if GetWindowRect(hwnd, &mut wr).is_ok()
                    && wr.right - wr.left > 0
                    && wr.bottom - wr.top > 0
                {
                    // When auto-hidden the tray window collapses to a sliver;
                    // still honour it (Mote can perch on the sliver's edge).
                    x = wr.left;
                    y = wr.top;
                    w = wr.right - wr.left;
                    h = wr.bottom - wr.top;
                }
            }
            let edge = match abd.uEdge {
                0 => TaskbarEdge::Left,   // ABE_LEFT
                1 => TaskbarEdge::Top,    // ABE_TOP
                2 => TaskbarEdge::Right,  // ABE_RIGHT
                _ => TaskbarEdge::Bottom, // ABE_BOTTOM
            };
            let edge = if autohide && (w < 8 || h < 8) {
                TaskbarEdge::Hidden
            } else {
                edge
            };
            let monitor =
                monitor_for_point(monitors, x + w / 2, y + h / 2).unwrap_or(fallback.monitor);
            return TaskbarInfo {
                x,
                y,
                w,
                h,
                edge,
                autohide,
                monitor,
            };
        }

        // ABM failed (rare): try the tray window directly.
        if let Ok(hwnd) = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) {
            let mut wr = RECT::default();
            if GetWindowRect(hwnd, &mut wr).is_ok() {
                let x = wr.left;
                let y = wr.top;
                let w = (wr.right - wr.left).max(1);
                let h = (wr.bottom - wr.top).max(1);
                let edge = if h <= w {
                    TaskbarEdge::Bottom
                } else {
                    TaskbarEdge::Left
                };
                let monitor =
                    monitor_for_point(monitors, x + w / 2, y + h / 2).unwrap_or(fallback.monitor);
                return TaskbarInfo {
                    x,
                    y,
                    w,
                    h,
                    edge,
                    autohide,
                    monitor,
                };
            }
        }
        fallback
    }
}

fn monitor_for_point(monitors: &[MonitorInfo], x: i32, y: i32) -> Option<u32> {
    monitors
        .iter()
        .position(|m| m.contains(x, y))
        .map(|i| i as u32)
}

fn fallback_taskbar(monitors: &[MonitorInfo]) -> TaskbarInfo {
    // Bottom 48px (logical) of the primary monitor, scaled by DPI.
    let (mx, my, mw, mh, dpi, idx) = monitors
        .iter()
        .enumerate()
        .find(|(_, m)| m.primary)
        .map(|(i, m)| (m.x, m.y, m.w, m.h, m.dpi, i as u32))
        .unwrap_or((0, 0, 1920, 1080, 96, 0));
    let h = ((48.0 * dpi as f32 / 96.0) as i32).clamp(30, 120);
    TaskbarInfo {
        x: mx,
        y: my + mh - h,
        w: mw,
        h,
        edge: TaskbarEdge::Bottom,
        autohide: false,
        monitor: idx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taskbar_query_returns_sane_rect() {
        let mons = crate::monitors::query_monitors();
        let t = query_taskbar(&mons);
        assert!(t.w > 0 && t.h > 0, "bad taskbar rect {t:?}");
    }

    #[test]
    fn fallback_is_bottom() {
        let mons = vec![MonitorInfo {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
            dpi: 96,
            primary: true,
        }];
        let t = fallback_taskbar(&mons);
        assert_eq!(t.edge, TaskbarEdge::Bottom);
        assert_eq!(t.stand_y(), Some(1080 - 48));
    }
}
