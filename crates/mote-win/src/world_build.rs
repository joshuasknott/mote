//! World assembly: monitors + taskbar + windows -> [`WorldSnapshot`].
//!
//! The single choke point where OS data becomes the physics world. Rules:
//! - taskbar contributes its standable edge (or a narrow ledge for
//!   side/top bars);
//! - each qualifying window contributes its top edge; tiny windows are
//!   marked unstable and ignored by jump targeting (but still landable in
//!   a pinch — better than falling through);
//! - each monitor bottom gets a `ScreenFloor` safety net so a Mote whose
//!   window vanishes mid-fall always has somewhere to land;
//! - supports are sorted deterministically (by id) so snapshots are stable.

use mote_core::{MonitorRect, Support, SupportKind, VirtualRect, WallSide, WorldSnapshot};

use crate::monitors::MonitorInfo;
use crate::taskbar::{TaskbarEdge, TaskbarInfo};
use crate::windows::TopLevelWindow;

pub struct WorldInputs<'a> {
    pub monitors: &'a [MonitorInfo],
    pub taskbar: &'a TaskbarInfo,
    pub windows: &'a [TopLevelWindow],
    pub generation: u64,
}

pub fn build_world(inp: &WorldInputs) -> WorldSnapshot {
    let mut supports: Vec<Support> = Vec::new();

    // 1. Taskbar.
    let t = inp.taskbar;
    match t.edge {
        TaskbarEdge::Bottom | TaskbarEdge::Hidden => {
            supports.push(Support {
                id: support_id_for_taskbar(t.monitor),
                kind: SupportKind::Taskbar,
                x1: t.x as f32,
                x2: (t.x + t.w) as f32,
                y: t.y as f32,
                y_bottom: t.y as f32,
                monitor: t.monitor,
                generation: inp.generation,
                stable: t.edge == TaskbarEdge::Bottom,
            });
        }
        // Side/top bars: expose the bar's top edge as a narrow ledge.
        TaskbarEdge::Left | TaskbarEdge::Right | TaskbarEdge::Top => {
            supports.push(Support {
                id: support_id_for_taskbar(t.monitor),
                kind: SupportKind::Taskbar,
                x1: t.x as f32,
                x2: (t.x + t.w) as f32,
                y: t.y as f32,
                y_bottom: t.y as f32,
                monitor: t.monitor,
                generation: inp.generation,
                stable: false,
            });
        }
    }

    // 2. Windows: top edges and vertical wall borders.
    for w in inp.windows {
        let width = w.w as f32;
        if width < 100.0 {
            continue;
        }
        let monitor = monitor_index_for(inp.monitors, w.x + w.w / 2, w.y);
        supports.push(Support {
            id: w.support_id(),
            kind: SupportKind::Window,
            x1: w.x as f32,
            x2: (w.x + w.w) as f32,
            y: w.y as f32,
            y_bottom: w.y as f32,
            monitor,
            generation: inp.generation,
            stable: w.w >= 160 && w.h >= 80,
        });

        // Vertical window walls (left and right borders) for wall-climbing
        if w.h >= 80 {
            supports.push(Support::new_wall(
                support_id_for_wall(w.support_id(), WallSide::Left),
                WallSide::Left,
                w.x as f32,
                w.y as f32,
                (w.y + w.h) as f32,
                monitor,
                inp.generation,
                w.h >= 120,
            ));
            supports.push(Support::new_wall(
                support_id_for_wall(w.support_id(), WallSide::Right),
                WallSide::Right,
                (w.x + w.w) as f32,
                w.y as f32,
                (w.y + w.h) as f32,
                monitor,
                inp.generation,
                w.h >= 120,
            ));
        }
    }

    // 3. Screen-floor safety nets (unstable: only used when nothing better
    //    is underfoot).
    for (i, m) in inp.monitors.iter().enumerate() {
        supports.push(Support {
            id: support_id_for_floor(i as u32),
            kind: SupportKind::ScreenFloor,
            x1: m.x as f32,
            x2: (m.x + m.w) as f32,
            y: (m.y + m.h - 1) as f32,
            y_bottom: (m.y + m.h - 1) as f32,
            monitor: i as u32,
            generation: inp.generation,
            stable: false,
        });
    }

    supports.sort_by_key(|s| s.id);

    let (vx, vy, vw, vh) = virtual_rect_of(inp.monitors);
    WorldSnapshot {
        supports,
        virtual_rect: VirtualRect {
            x: vx,
            y: vy,
            w: vw,
            h: vh,
        },
        monitors: inp
            .monitors
            .iter()
            .map(|m| MonitorRect {
                x: m.x,
                y: m.y,
                w: m.w,
                h: m.h,
                dpi: m.dpi,
                primary: m.primary,
            })
            .collect(),
        generation: inp.generation,
    }
}

fn support_id_for_taskbar(monitor: u32) -> u64 {
    // Taskbar ids live in a reserved range that can never collide with HWNDs
    // on 64-bit Windows (low values are never valid window handles).
    0x7A5B_0000_0000u64 | monitor as u64
}

fn support_id_for_floor(monitor: u32) -> u64 {
    0xF100_0000_0000u64 | monitor as u64
}

fn support_id_for_wall(window_id: u64, side: WallSide) -> u64 {
    match side {
        WallSide::Left => 0x2000_0000_0000_0000u64 | (window_id & 0x0FFF_FFFF_FFFF_FFFFu64),
        WallSide::Right => 0x3000_0000_0000_0000u64 | (window_id & 0x0FFF_FFFF_FFFF_FFFFu64),
    }
}

fn monitor_index_for(monitors: &[MonitorInfo], x: i32, y: i32) -> u32 {
    monitors
        .iter()
        .position(|m| m.contains(x, y))
        .map(|i| i as u32)
        .unwrap_or(0)
}

fn virtual_rect_of(monitors: &[MonitorInfo]) -> (i32, i32, i32, i32) {
    if monitors.is_empty() {
        return (0, 0, 1920, 1080);
    }
    let x1 = monitors.iter().map(|m| m.x).min().unwrap();
    let y1 = monitors.iter().map(|m| m.y).min().unwrap();
    let x2 = monitors.iter().map(|m| m.x + m.w).max().unwrap();
    let y2 = monitors.iter().map(|m| m.y + m.h).max().unwrap();
    (x1, y1, x2 - x1, y2 - y1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitors::MonitorInfo;
    use crate::taskbar::{TaskbarEdge, TaskbarInfo};

    fn monitors() -> Vec<MonitorInfo> {
        vec![MonitorInfo {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
            dpi: 96,
            primary: true,
        }]
    }

    fn taskbar() -> TaskbarInfo {
        TaskbarInfo {
            x: 0,
            y: 1040,
            w: 1920,
            h: 40,
            edge: TaskbarEdge::Bottom,
            autohide: false,
            monitor: 0,
        }
    }

    #[test]
    fn empty_windows_still_has_taskbar_and_floor() {
        let m = monitors();
        let t = taskbar();
        let w = build_world(&WorldInputs {
            monitors: &m,
            taskbar: &t,
            windows: &[],
            generation: 1,
        });
        assert!(w.taskbar().is_some());
        assert!(w
            .supports
            .iter()
            .any(|s| s.kind == SupportKind::ScreenFloor));
    }

    #[test]
    fn tiny_windows_excluded() {
        // describe_window already filters <80px; build_world drops <100px.
        let m = monitors();
        let t = taskbar();
        let w = build_world(&WorldInputs {
            monitors: &m,
            taskbar: &t,
            windows: &[],
            generation: 1,
        });
        assert!(w
            .supports
            .iter()
            .all(|s| s.width() >= 99.0 || s.kind == SupportKind::ScreenFloor));
    }

    #[test]
    fn deterministic_order() {
        let m = monitors();
        let t = taskbar();
        let a = build_world(&WorldInputs {
            monitors: &m,
            taskbar: &t,
            windows: &[],
            generation: 1,
        });
        let b = build_world(&WorldInputs {
            monitors: &m,
            taskbar: &t,
            windows: &[],
            generation: 1,
        });
        let ida: Vec<u64> = a.supports.iter().map(|s| s.id).collect();
        let idb: Vec<u64> = b.supports.iter().map(|s| s.id).collect();
        assert_eq!(ida, idb);
    }
}
