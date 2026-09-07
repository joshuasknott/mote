//! 2D platformer-lite physics for the desktop world.
//!
//! Coordinates are desktop virtual-screen pixels, y growing downwards
//! (matching Win32 screen space). The creature's position is its **feet
//! centre**: the point that must rest on top of a support surface.
//!
//! Deterministic: no randomness, no wall-clock access. Fixed-timestep
//! friendly and fully unit-tested.

use crate::world::{SupportKind, WorldSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Tunable simulation constants.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SimConfig {
    pub gravity_px_s2: f32,
    pub walk_speed: f32,
    pub run_speed: f32,
    pub jump_velocity: f32,
    pub max_fall_speed: f32,
    /// Max horizontal reach of a jump, pixels.
    pub max_jump_dist: f32,
    /// Max height Mote can jump upward, pixels.
    pub max_jump_up: f32,
    /// How far below the feet a support may be and still count as grounded.
    pub ground_snap: f32,
    /// Horizontal friction applied when no locomotion intent (px/s^2).
    pub ground_friction: f32,
    pub air_drag: f32,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            gravity_px_s2: 2800.0,
            walk_speed: 95.0,
            run_speed: 265.0,
            jump_velocity: 880.0,
            max_fall_speed: 1500.0,
            max_jump_dist: 300.0,
            max_jump_up: 165.0,
            ground_snap: 6.0,
            ground_friction: 1400.0,
            air_drag: 0.4,
        }
    }
}

/// Physics body of one creature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    /// Feet-centre position in virtual-screen pixels.
    pub pos: Vec2,
    pub vel: Vec2,
    /// -1 facing left, +1 facing right.
    pub facing: i8,
    pub grounded_surface: Option<u64>,
    pub airborne_time_s: f32,
    /// Approximate body width used for edge detection.
    pub half_width: f32,
}

impl Body {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            pos: Vec2::new(x, y),
            vel: Vec2::ZERO,
            facing: 1,
            grounded_surface: None,
            airborne_time_s: 0.0,
            half_width: 16.0,
        }
    }

    pub fn grounded(&self) -> bool {
        self.grounded_surface.is_some()
    }

    pub fn set_horizontal_target(&mut self, _target: Option<f32>, cfg: &SimConfig) {
        // Friction-stop: decay horizontal velocity toward zero.
        let f = cfg.ground_friction * (1.0 / 60.0);
        if self.vel.x.abs() <= f {
            self.vel.x = 0.0;
        } else {
            self.vel.x -= self.vel.x.signum() * f;
        }
    }

    /// Begin a jump arc aimed at (`tx`, `ty`). Picks a horizontal velocity
    /// that roughly reaches the target and a vertical velocity scaled by the
    /// height difference. Deterministic, no randomness.
    pub fn start_jump_toward(&mut self, tx: f32, ty: f32, cfg: &SimConfig) {
        let dx = tx - self.pos.x;
        let dy = ty - self.pos.y; // negative = upward
        let dist = dx.abs().clamp(20.0, cfg.max_jump_dist);
        // Time of flight estimate: longer for longer jumps.
        let t = (dist / cfg.run_speed).clamp(0.35, 0.8);
        let vx = (dx / t).clamp(-cfg.run_speed * 1.15, cfg.run_speed * 1.15);
        // y(t) = y0 + vy*t + g/2 t^2  =>  vy = (dy - g/2 t^2)/t
        let vy = ((dy - 0.5 * cfg.gravity_px_s2 * t * t) / t)
            .clamp(-cfg.jump_velocity * 1.2, cfg.jump_velocity * 0.4);
        self.vel.x = vx;
        self.vel.y = vy.min(-220.0);
        self.facing = if vx >= 0.0 { 1 } else { -1 };
        self.grounded_surface = None;
        self.airborne_time_s = 0.0;
    }

    pub fn throw_with(&mut self, vx: f32, vy: f32) {
        self.vel.x = vx.clamp(-1200.0, 1200.0);
        self.vel.y = vy.clamp(-1200.0, 600.0);
        self.grounded_surface = None;
        self.airborne_time_s = 0.0;
    }

    /// Integrate one step. Returns discrete events for the behaviour layer.
    pub fn integrate(
        &mut self,
        dt: f32,
        world: &WorldSnapshot,
        cfg: &SimConfig,
    ) -> Vec<PhysicsEvent> {
        let mut events = Vec::new();
        let was_grounded = self.grounded_surface;

        // Validate current support first: windows move/disappear under Mote.
        if let Some(id) = self.grounded_surface {
            match world.support(id) {
                Some(s) if s.is_wall() => {
                    self.pos.x = s.x1;
                    let min_y = s.y.min(s.y_bottom);
                    let max_y = s.y.max(s.y_bottom);
                    if self.pos.y < min_y - 20.0 || self.pos.y > max_y + 20.0 {
                        self.grounded_surface = None;
                        self.airborne_time_s = 0.0;
                        events.push(PhysicsEvent::SupportLost { surface: id });
                    }
                }
                Some(s) => {
                    // Support moved away horizontally or vertically?
                    if self.pos.x < s.x1 - self.half_width
                        || self.pos.x > s.x2 + self.half_width
                        || (self.pos.y - s.y).abs() > cfg.ground_snap + 14.0
                    {
                        self.grounded_surface = None;
                        self.airborne_time_s = 0.0;
                        events.push(PhysicsEvent::SupportLost { surface: id });
                    } else {
                        // Ride moving supports: stick feet to the surface top.
                        self.pos.y = s.y;
                    }
                }
                None => {
                    self.grounded_surface = None;
                    self.airborne_time_s = 0.0;
                    events.push(PhysicsEvent::SupportLost { surface: id });
                }
            }
        }

        if self.grounded_surface.is_some() {
            let is_wall = world
                .support(self.grounded_surface.unwrap())
                .map(|s| s.is_wall())
                .unwrap_or(false);

            if is_wall {
                let wall = world.support(self.grounded_surface.unwrap()).unwrap();
                self.pos.x = wall.x1;
                self.pos.y += self.vel.y * dt;
                self.airborne_time_s = 0.0;
                let vr = &world.virtual_rect;
                self.pos.y = self.pos.y.clamp(vr.y as f32, (vr.y + vr.h) as f32);
            } else {
                // Grounded: horizontal velocity is owned by the behaviour layer
                // (already written into vel). Just advance and check edges.
                self.pos.x += self.vel.x * dt;
                self.pos.y = world
                    .support(self.grounded_surface.unwrap())
                    .map(|s| s.y)
                    .unwrap_or(self.pos.y);
                self.airborne_time_s = 0.0;

                // Edge detection: walked past the end of the support?
                if let Some(id) = self.grounded_surface {
                    if let Some(s) = world.support(id) {
                        // Allow a small overhang before tipping off — reads as
                        // "peering over the edge" in the renderer.
                        let overhang = 6.0;
                        if self.pos.x < s.x1 - overhang || self.pos.x > s.x2 + overhang {
                            self.grounded_surface = None;
                            self.airborne_time_s = 0.0001;
                            events.push(PhysicsEvent::FellOffEdge { surface: id });
                        }
                    }
                }
                // Screen-edge walls: don't walk off the virtual desktop sides.
                let vr = &world.virtual_rect;
                if self.pos.x < vr.x as f32 {
                    self.pos.x = vr.x as f32;
                    self.vel.x = 0.0;
                    events.push(PhysicsEvent::HitWall { at_x: self.pos.x });
                } else if self.pos.x > (vr.x + vr.w) as f32 {
                    self.pos.x = (vr.x + vr.w) as f32;
                    self.vel.x = 0.0;
                    events.push(PhysicsEvent::HitWall { at_x: self.pos.x });
                }
            }
        } else {
            // Airborne: gravity + drag, then swept landing check.
            self.airborne_time_s += dt;
            self.vel.y = (self.vel.y + cfg.gravity_px_s2 * dt).min(cfg.max_fall_speed);
            self.vel.x -= self.vel.x * cfg.air_drag * dt;

            let prev_y = self.pos.y;
            self.pos.x += self.vel.x * dt;
            self.pos.y += self.vel.y * dt;

            // Clamp to virtual screen sides while airborne.
            let vr = &world.virtual_rect;
            if self.pos.x < vr.x as f32 {
                self.pos.x = vr.x as f32;
                self.vel.x = 0.0;
            } else if self.pos.x > (vr.x + vr.w) as f32 {
                self.pos.x = (vr.x + vr.w) as f32;
                self.vel.x = 0.0;
            }

            // Landing: only when moving downward and crossing a support top.
            if self.vel.y >= 0.0 {
                if let Some(s) = world.first_support_crossed(self.pos.x, prev_y, self.pos.y) {
                    // Desktop floor safety net: if it is the bottom of the
                    // virtual screen with no taskbar, still land.
                    self.pos.y = s.y;
                    self.grounded_surface = Some(s.id);
                    let impact = self.vel.y;
                    self.vel.y = 0.0;
                    self.vel.x *= 0.35; // landing scrub
                    self.airborne_time_s = 0.0;
                    events.push(PhysicsEvent::Landed {
                        surface: s.id,
                        impact_px_s: impact,
                        kind: s.kind,
                    });
                }
            }

            // Fell below the world entirely (support vanished mid-fall):
            // recover onto the lowest known support or screen bottom.
            let bottom = (vr.y + vr.h) as f32;
            if self.pos.y > bottom + 40.0 {
                if let Some(s) = world.support_below(self.pos.x, bottom - 1.0, 4000.0) {
                    self.pos.x = self.pos.x.clamp(s.x1, s.x2);
                    self.pos.y = s.y;
                    self.grounded_surface = Some(s.id);
                    self.vel = Vec2::ZERO;
                    events.push(PhysicsEvent::Recovered { surface: s.id });
                } else {
                    self.pos.y = bottom - 2.0;
                    self.vel = Vec2::ZERO;
                    events.push(PhysicsEvent::Recovered { surface: u64::MAX });
                }
            }
        }

        if was_grounded.is_none() && events.is_empty() {
            // Landed exactly on snap boundary without crossing (e.g. thrown
            // gently onto a surface): still report it.
            if let Some(surface) = self.grounded_surface {
                events.push(PhysicsEvent::Landed {
                    surface,
                    impact_px_s: 0.0,
                    kind: SupportKind::Window,
                });
            }
        }

        events
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PhysicsEvent {
    Landed {
        surface: u64,
        impact_px_s: f32,
        kind: SupportKind,
    },
    FellOffEdge {
        surface: u64,
    },
    SupportLost {
        surface: u64,
    },
    HitWall {
        at_x: f32,
    },
    Recovered {
        surface: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{MonitorRect, Support, SupportKind, VirtualRect, WorldSnapshot};

    fn flat_world() -> WorldSnapshot {
        WorldSnapshot {
            supports: vec![Support {
                id: 1,
                kind: SupportKind::Taskbar,
                x1: 0.0,
                x2: 1920.0,
                y: 1040.0,
                y_bottom: 1040.0,
                monitor: 0,
                generation: 1,
                stable: true,
            }],
            virtual_rect: VirtualRect {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
            },
            monitors: vec![MonitorRect {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
                dpi: 96,
                primary: true,
            }],
            generation: 1,
        }
    }

    #[test]
    fn falls_and_lands_on_taskbar() {
        let world = flat_world();
        let cfg = SimConfig::default();
        let mut b = Body::new(500.0, 200.0);
        let mut landed = false;
        for _ in 0..240 {
            let ev = b.integrate(1.0 / 60.0, &world, &cfg);
            if ev.iter().any(|e| matches!(e, PhysicsEvent::Landed { .. })) {
                landed = true;
                break;
            }
        }
        assert!(landed, "should land on the taskbar");
        assert!((b.pos.y - 1040.0).abs() < 0.01);
        assert!(b.grounded());
    }

    #[test]
    fn walking_off_edge_starts_fall() {
        let world = WorldSnapshot {
            supports: vec![Support {
                id: 7,
                kind: SupportKind::Window,
                x1: 100.0,
                x2: 400.0,
                y: 500.0,
                y_bottom: 500.0,
                monitor: 0,
                generation: 1,
                stable: true,
            }],
            virtual_rect: VirtualRect {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
            },
            monitors: vec![],
            generation: 1,
        };
        let cfg = SimConfig::default();
        let mut b = Body::new(390.0, 500.0);
        b.grounded_surface = Some(7);
        b.vel.x = 95.0;
        let mut fell = false;
        for _ in 0..120 {
            let ev = b.integrate(1.0 / 60.0, &world, &cfg);
            if ev
                .iter()
                .any(|e| matches!(e, PhysicsEvent::FellOffEdge { .. }))
            {
                fell = true;
                break;
            }
        }
        assert!(fell);
        assert!(!b.grounded());
    }

    #[test]
    fn support_removed_reports_support_lost() {
        let world = flat_world();
        let cfg = SimConfig::default();
        let mut b = Body::new(500.0, 1040.0);
        b.grounded_surface = Some(1);
        let empty = WorldSnapshot {
            supports: vec![],
            virtual_rect: world.virtual_rect,
            monitors: vec![],
            generation: 2,
        };
        let ev = b.integrate(1.0 / 60.0, &empty, &cfg);
        assert!(ev
            .iter()
            .any(|e| matches!(e, PhysicsEvent::SupportLost { surface: 1 })));
        assert!(!b.grounded());
    }

    #[test]
    fn rides_moving_support() {
        let mut world = flat_world();
        let cfg = SimConfig::default();
        let mut b = Body::new(500.0, 1040.0);
        b.grounded_surface = Some(1);
        // Window/taskbar moved up by 20px: Mote should ride it, not fall.
        world.supports[0].y = 1020.0;
        let ev = b.integrate(1.0 / 60.0, &world, &cfg);
        assert!(!ev.iter().any(|e| matches!(
            e,
            PhysicsEvent::SupportLost { .. } | PhysicsEvent::FellOffEdge { .. }
        )));
        assert!((b.pos.y - 1020.0).abs() < 0.01);
    }

    #[test]
    fn jump_toward_reaches_nearby_ledge() {
        let world = WorldSnapshot {
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
                    x1: 600.0,
                    x2: 1000.0,
                    y: 900.0,
                    y_bottom: 900.0,
                    monitor: 0,
                    generation: 1,
                    stable: true,
                },
            ],
            virtual_rect: VirtualRect {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
            },
            monitors: vec![],
            generation: 1,
        };
        let cfg = SimConfig::default();
        let mut b = Body::new(500.0, 1040.0);
        b.grounded_surface = Some(1);
        b.start_jump_toward(800.0, 900.0, &cfg);
        let mut landed_on_2 = false;
        for _ in 0..180 {
            let ev = b.integrate(1.0 / 60.0, &world, &cfg);
            if ev
                .iter()
                .any(|e| matches!(e, PhysicsEvent::Landed { surface: 2, .. }))
            {
                landed_on_2 = true;
                break;
            }
            if b.grounded() && b.grounded_surface != Some(2) && b.pos.y >= 1039.0 {
                break; // fell back; acceptable only if jump was impossible
            }
        }
        assert!(landed_on_2, "jump must reach and land on ledge 2");
    }

    #[test]
    fn vertical_wall_climbing_crawls_and_sticks_to_edge() {
        use crate::world::WallSide;
        let wall = Support::new_wall(5, WallSide::Left, 400.0, 300.0, 800.0, 0, 1, true);
        let world = WorldSnapshot {
            supports: vec![wall],
            virtual_rect: VirtualRect {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
            },
            monitors: vec![],
            generation: 1,
        };
        let cfg = SimConfig::default();
        let mut b = Body::new(400.0, 600.0);
        b.grounded_surface = Some(5);
        b.vel.y = -100.0; // Crawling upward
        for _ in 0..60 {
            b.integrate(1.0 / 60.0, &world, &cfg);
        }
        assert_eq!(b.pos.x, 400.0, "body x must remain locked to the wall");
        assert!(
            b.pos.y < 550.0,
            "body must have crawled upward: {}",
            b.pos.y
        );
        assert!(b.grounded(), "body must remain grounded on the wall");
    }

    #[test]
    fn deterministic_repeat() {
        let world = flat_world();
        let cfg = SimConfig::default();
        let run = || {
            let mut b = Body::new(500.0, 200.0);
            b.vel.x = 30.0;
            for _ in 0..90 {
                b.integrate(1.0 / 60.0, &world, &cfg);
            }
            (b.pos.x, b.pos.y)
        };
        assert_eq!(run(), run());
    }
}
