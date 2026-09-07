//! Physical desktop world model: the set of surfaces Mote can stand on.
//!
//! Built by `mote-win` from native Windows queries (monitors, taskbar,
//! top-level windows) and consumed by physics + behaviour. Plain data only —
//! no OS calls — so tests can fabricate desktops freely.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WallSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SupportKind {
    Taskbar,
    Window,
    /// Bottom of the virtual screen when nothing else is there (safety net).
    ScreenFloor,
    /// Vertical window edge (left or right border) for wall-climbing.
    WindowWall {
        side: WallSide,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Support {
    /// Stable-ish id derived from the window handle (see mote-win).
    pub id: u64,
    pub kind: SupportKind,
    /// Standable horizontal span, virtual-screen pixels.
    pub x1: f32,
    pub x2: f32,
    /// Y of the top surface (feet rest here).
    pub y: f32,
    /// Y of the bottom surface (for vertical walls; defaults to y for horizontal ledges).
    #[serde(default)]
    pub y_bottom: f32,
    pub monitor: u32,
    /// Bumped whenever the underlying window moves/resizes.
    pub generation: u64,
    /// False for very small / transient windows Mote should avoid.
    pub stable: bool,
}

impl Support {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u64,
        kind: SupportKind,
        x1: f32,
        x2: f32,
        y: f32,
        monitor: u32,
        generation: u64,
        stable: bool,
    ) -> Self {
        Self {
            id,
            kind,
            x1,
            x2,
            y,
            y_bottom: y,
            monitor,
            generation,
            stable,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_wall(
        id: u64,
        side: WallSide,
        x: f32,
        y_top: f32,
        y_bottom: f32,
        monitor: u32,
        generation: u64,
        stable: bool,
    ) -> Self {
        Self {
            id,
            kind: SupportKind::WindowWall { side },
            x1: x,
            x2: x,
            y: y_top,
            y_bottom,
            monitor,
            generation,
            stable,
        }
    }

    pub fn is_wall(&self) -> bool {
        matches!(self.kind, SupportKind::WindowWall { .. })
    }

    pub fn wall_side(&self) -> Option<WallSide> {
        match self.kind {
            SupportKind::WindowWall { side } => Some(side),
            _ => None,
        }
    }

    pub fn height(&self) -> f32 {
        (self.y_bottom - self.y).abs().max(0.0)
    }

    pub fn contains_y(&self, y: f32, tol: f32) -> bool {
        let min_y = self.y.min(self.y_bottom);
        let max_y = self.y.max(self.y_bottom);
        y >= min_y - tol && y <= max_y + tol
    }

    pub fn width(&self) -> f32 {
        (self.x2 - self.x1).max(0.0)
    }
    pub fn center_x(&self) -> f32 {
        (self.x1 + self.x2) * 0.5
    }
    pub fn contains_x(&self, x: f32, tol: f32) -> bool {
        x >= self.x1 - tol && x <= self.x2 + tol
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub dpi: u32,
    pub primary: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub supports: Vec<Support>,
    pub virtual_rect: VirtualRect,
    pub monitors: Vec<MonitorRect>,
    /// Bumped whenever the support set materially changes.
    pub generation: u64,
}

impl Default for VirtualRect {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        }
    }
}

impl WorldSnapshot {
    pub fn empty() -> Self {
        Self {
            supports: Vec::new(),
            virtual_rect: VirtualRect::default(),
            monitors: Vec::new(),
            generation: 0,
        }
    }

    pub fn support(&self, id: u64) -> Option<&Support> {
        self.supports.iter().find(|s| s.id == id)
    }

    /// Support whose top surface contains `x` and sits within `snap` px of `y`.
    pub fn query_support_at(&self, x: f32, y: f32, snap: f32) -> Option<&Support> {
        self.supports
            .iter()
            .filter(|s| !s.is_wall() && s.contains_x(x, 2.0) && (s.y - y).abs() <= snap)
            .max_by(|a, b| {
                // Prefer the highest surface at/below the query point, then widest.
                let ka = (a.y <= y + snap) as u8;
                let kb = (b.y <= y + snap) as u8;
                ka.cmp(&kb)
                    .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
                    .then(
                        a.width()
                            .partial_cmp(&b.width())
                            .unwrap_or(std::cmp::Ordering::Equal),
                    )
            })
    }

    /// Nearest support at or below `y` containing `x` (used for falling).
    pub fn support_below(&self, x: f32, y: f32, max_fall: f32) -> Option<&Support> {
        self.supports
            .iter()
            .filter(|s| {
                !s.is_wall() && s.contains_x(x, 1.0) && s.y >= y - 1.0 && s.y - y <= max_fall
            })
            .min_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Swept variant: first support top crossed while falling from `prev_y`
    /// to `new_y` at horizontal `x`.
    pub fn first_support_crossed(&self, x: f32, prev_y: f32, new_y: f32) -> Option<&Support> {
        self.supports
            .iter()
            .filter(|s| {
                !s.is_wall() && s.contains_x(x, 1.0) && s.y >= prev_y - 0.5 && s.y <= new_y + 0.5
            })
            .min_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Nearest vertical window wall containing `y` and within `tol` horizontal px of `x`.
    pub fn query_wall_at(&self, x: f32, y: f32, tol: f32) -> Option<&Support> {
        self.supports
            .iter()
            .filter(|s| s.is_wall() && s.contains_y(y, 16.0) && (s.x1 - x).abs() <= tol)
            .min_by(|a, b| {
                (a.x1 - x)
                    .abs()
                    .partial_cmp(&(b.x1 - x).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Finds any window wall near `pos` (within `reach_dist` px) suitable for climbing.
    pub fn wall_near(&self, x: f32, y: f32, reach_dist: f32) -> Option<&Support> {
        self.supports
            .iter()
            .filter(|s| s.is_wall() && s.contains_y(y, 24.0) && (s.x1 - x).abs() <= reach_dist)
            .min_by(|a, b| {
                (a.x1 - x)
                    .abs()
                    .partial_cmp(&(b.x1 - x).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Distance from `x` to the nearest edge of support `id`. Negative means
    /// already past the edge. Also returns which side (-1 left, +1 right).
    pub fn edge_info(&self, id: u64, x: f32) -> Option<(f32, i8)> {
        let s = self.support(id)?;
        let dl = x - s.x1;
        let dr = s.x2 - x;
        if dl < dr {
            Some((dl, -1))
        } else {
            Some((dr, 1))
        }
    }

    /// Candidate jump/climb targets from `from` given physical capability.
    /// Sorted best-first: near, wide, stable, on the same monitor.
    pub fn jump_targets(
        &self,
        from: (f32, f32),
        current: Option<u64>,
        max_dist: f32,
        max_up: f32,
    ) -> Vec<&Support> {
        let (fx, fy) = from;
        let mut out: Vec<&Support> = self
            .supports
            .iter()
            .filter(|s| {
                !s.is_wall()
                    && Some(s.id) != current
                    && s.stable
                    && s.width() >= 90.0
                    && (s.y - fy) <= 700.0 // don't dive absurdly far
                    && (fy - s.y) <= max_up
                    && ((s.center_x() - fx).abs() - s.width() * 0.5).max(0.0) <= max_dist
            })
            .collect();
        out.sort_by(|a, b| {
            let score = |s: &&Support| {
                let gap = ((s.center_x() - fx).abs() - s.width() * 0.5).max(0.0);
                let rise = (fy - s.y).max(0.0);
                gap + rise * 1.5
                    - s.width() * 0.05
                    - if s.kind == SupportKind::Taskbar {
                        20.0
                    } else {
                        0.0
                    }
            };
            score(a)
                .partial_cmp(&score(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    /// Highest taskbar support, if any.
    pub fn taskbar(&self) -> Option<&Support> {
        self.supports
            .iter()
            .filter(|s| s.kind == SupportKind::Taskbar)
            .max_by(|a, b| {
                a.width()
                    .partial_cmp(&b.width())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> WorldSnapshot {
        WorldSnapshot {
            supports: vec![
                Support {
                    id: 1,
                    kind: SupportKind::Taskbar,
                    x1: 0.0,
                    x2: 1920.0,
                    y: 1040.0,
                    y_bottom: 1040.0,
                    monitor: 0,
                    generation: 1,
                    stable: true,
                },
                Support {
                    id: 2,
                    kind: SupportKind::Window,
                    x1: 200.0,
                    x2: 800.0,
                    y: 600.0,
                    y_bottom: 600.0,
                    monitor: 0,
                    generation: 1,
                    stable: true,
                },
                Support::new_wall(3, WallSide::Left, 200.0, 600.0, 900.0, 0, 1, true),
                Support::new_wall(4, WallSide::Right, 800.0, 600.0, 900.0, 0, 1, true),
            ],
            virtual_rect: VirtualRect {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
            },
            monitors: vec![],
            generation: 1,
        }
    }

    #[test]
    fn query_prefers_window_over_taskbar() {
        let w = world();
        let s = w.query_support_at(400.0, 600.0, 8.0).unwrap();
        assert_eq!(s.id, 2);
    }

    #[test]
    fn support_below_finds_taskbar() {
        let w = world();
        let s = w.support_below(1500.0, 700.0, 2000.0).unwrap();
        assert_eq!(s.id, 1);
    }

    #[test]
    fn jump_targets_ranked() {
        let w = world();
        let t = w.jump_targets((400.0, 1040.0), Some(1), 600.0, 500.0);
        assert!(!t.is_empty());
        assert_eq!(t[0].id, 2);
    }

    #[test]
    fn wall_query_detects_vertical_borders() {
        let w = world();
        let left = w.query_wall_at(204.0, 750.0, 10.0).unwrap();
        assert_eq!(left.id, 3);
        assert_eq!(left.wall_side(), Some(WallSide::Left));
        assert!(left.is_wall());

        let right = w.wall_near(820.0, 800.0, 30.0).unwrap();
        assert_eq!(right.id, 4);
        assert_eq!(right.wall_side(), Some(WallSide::Right));
    }
}
