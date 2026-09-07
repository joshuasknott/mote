//! mote-core: platform-independent simulation of the desktop as a small 2D
//! physical world, plus the creature's behaviour system.
//!
//! Strictly no Windows API calls here. Everything this crate needs from the
//! OS arrives as plain data ([`WorldSnapshot`], [`SenseInput`]) so the
//! simulation stays deterministic and unit-testable.
//!
//! Future-proofing: [`WorldSnapshot`] and [`CreatureSim`] are written so a
//! future version can simulate N creatures sharing the same surfaces (see
//! `docs/ARCHITECTURE.md`). For now the app runs exactly one creature.

pub mod behaviour;
pub mod personality;
pub mod physics;
pub mod species;
pub mod world;

pub use behaviour::{BehaviourState, Brain, DecisionContext, Intent, IntentKind};
pub use personality::{Drives, Personality};
pub use physics::{Body, PhysicsEvent, SimConfig, Vec2};
pub use species::SpeciesId;
pub use world::{MonitorRect, Support, SupportKind, VirtualRect, WallSide, WorldSnapshot};

use serde::{Deserialize, Serialize};

/// Full per-creature simulation state. Owns a physics body, a behaviour brain
/// and the internal drives. The app ticks this at a fixed timestep.
#[derive(Debug, Clone)]
pub struct CreatureSim {
    pub id: u32,
    pub species: SpeciesId,
    pub body: Body,
    pub brain: Brain,
    pub drives: Drives,
    pub personality: Personality,
    /// Last intent chosen by the brain (for rendering / debugging).
    pub last_intent: Intent,
    /// Monotonic clock of the last tick, milliseconds.
    pub last_tick_ms: u64,
}

impl CreatureSim {
    pub fn new(id: u32, x: f32, y: f32, now_ms: u64) -> Self {
        Self::new_with_species(id, SpeciesId::default(), x, y, now_ms)
    }

    pub fn new_with_species(id: u32, species: SpeciesId, x: f32, y: f32, now_ms: u64) -> Self {
        Self {
            id,
            species,
            body: Body::new(x, y),
            brain: Brain::new(now_ms),
            drives: Drives::default(),
            personality: species.default_personality(),
            last_intent: Intent::stay(),
            last_tick_ms: now_ms,
        }
    }

    pub fn set_species(&mut self, species: SpeciesId) {
        self.species = species;
        self.personality = species.default_personality();
    }

    /// Advance the simulation by `dt` seconds.
    ///
    /// `sense` carries cheap per-tick inputs (cursor, cpu, audio...).
    /// `world` is the latest desktop world model (refreshed at ~4 Hz).
    /// Returns the physics events that occurred this tick.
    pub fn tick(
        &mut self,
        dt: f32,
        now_ms: u64,
        sense: &SenseInput,
        world: &WorldSnapshot,
        cfg: &SimConfig,
    ) -> Vec<PhysicsEvent> {
        let dt = dt.clamp(0.0005, 0.05);
        self.last_tick_ms = now_ms;

        // 1. Update internal drives from the environment.
        self.drives.update(
            dt,
            sense.idle_ms as f32 / 1000.0,
            sense.user_active,
            sense.media_playing,
            sense.cpu_01,
            self.brain.state == BehaviourState::Sleep,
            &self.personality,
        );

        // 2. Ask the brain what to do.
        let ctx = DecisionContext::from_sense(sense, &self.body, &self.drives, &self.personality);
        let intent = self
            .brain
            .update(now_ms, dt, &ctx, &self.body, world, &self.drives);
        self.last_intent = intent.clone();

        // 3. Convert the intent into locomotion targets for the physics body,
        //    then integrate.
        apply_intent_to_body(&mut self.body, &intent, world, cfg, dt);
        let events = self.body.integrate(dt, world, cfg);

        // 4. Let the brain observe physics outcomes (landing, losing support).
        self.brain.observe_physics(&events, now_ms);

        events
    }
}

fn apply_intent_to_body(
    body: &mut Body,
    intent: &Intent,
    world: &WorldSnapshot,
    cfg: &SimConfig,
    _dt: f32,
) {
    use IntentKind as K;
    match intent.kind {
        K::Stay
        | K::Sit
        | K::Sleep
        | K::LookAt
        | K::WatchWindow
        | K::Dance
        | K::ReactLoad
        | K::Peek => {
            body.set_horizontal_target(None, cfg);
            body.vel.y = 0.0;
        }
        K::WalkTo | K::RunTo | K::ChaseCursor | K::WanderTo => {
            let speed = match intent.kind {
                K::RunTo | K::ChaseCursor => cfg.run_speed,
                _ => cfg.walk_speed,
            };
            let dir = (intent.target_x - body.pos.x).signum();
            if intent.target_x.is_finite() && dir != 0.0 {
                body.vel.x = dir * speed;
                body.facing = if dir > 0.0 { 1 } else { -1 };
            } else {
                body.set_horizontal_target(None, cfg);
            }
        }
        K::AvoidCursor => {
            // Run away from the cursor horizontally on the current support.
            let dir = (body.pos.x - intent.target_x).signum();
            let dir = if dir == 0.0 { body.facing as f32 } else { dir };
            body.vel.x = dir * cfg.run_speed;
            body.facing = if dir > 0.0 { 1 } else { -1 };
        }
        K::JumpTo => {
            if let Some(id) = intent.target_surface {
                if let Some(s) = world.support(id) {
                    body.start_jump_toward(s.center_x(), s.y, cfg);
                }
            }
            // Keep current horizontal velocity while airborne.
        }
        K::ClimbTo => {
            // Wall-climbing locomotion: if on a vertical wall, climb upward/downward.
            if let Some(id) = body.grounded_surface {
                if let Some(s) = world.support(id) {
                    if s.is_wall() {
                        body.pos.x = s.x1;
                        if let Some(side) = s.wall_side() {
                            body.facing = match side {
                                WallSide::Left => 1,
                                WallSide::Right => -1,
                            };
                        }
                        let dy = intent.target_y - body.pos.y;
                        body.vel.x = 0.0;
                        if dy.abs() > 4.0 {
                            body.vel.y = if dy < 0.0 { -75.0 } else { 55.0 };
                        } else {
                            body.vel.y = -55.0;
                        }
                        // Reached top of the wall? Scramble onto the top window surface!
                        if body.pos.y <= s.y + 8.0 {
                            let top_x = if s.wall_side() == Some(WallSide::Left) {
                                s.x1 + 18.0
                            } else {
                                s.x1 - 18.0
                            };
                            if let Some(top_s) = world.query_support_at(top_x, s.y, 14.0) {
                                body.pos.x = top_x;
                                body.pos.y = top_s.y;
                                body.grounded_surface = Some(top_s.id);
                                body.vel.y = 0.0;
                            }
                        }
                        return;
                    }
                }
            }
            // If near a wall and below its top (or airborne), latch onto it
            if let Some(wall) = world.wall_near(body.pos.x, body.pos.y, 50.0) {
                if !body.grounded() || body.pos.y > wall.y + 16.0 {
                    body.pos.x = wall.x1;
                    body.grounded_surface = Some(wall.id);
                    body.vel.x = 0.0;
                    body.vel.y = -75.0;
                    if let Some(side) = wall.wall_side() {
                        body.facing = match side {
                            WallSide::Left => 1,
                            WallSide::Right => -1,
                        };
                    }
                }
            } else if let Some(id) = intent.target_surface {
                if let Some(s) = world.support(id) {
                    body.start_jump_toward(s.center_x(), s.y, cfg);
                }
            }
        }
        K::Dragged => {
            // While dragged the app sets body.pos directly; kill velocity so
            // release throws are computed from the drag trail instead.
            body.vel.x = 0.0;
            body.vel.y = 0.0;
        }
    }
    // Clamp facing to unit.
    if body.facing == 0 {
        body.facing = 1;
    }
}

/// Cheap per-tick sensory input. Built by `mote-win` (production) or by tests.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SenseInput {
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub cursor_vx: f32,
    pub cursor_vy: f32,
    pub cursor_speed_px_s: f32,
    /// How long since the last input event, milliseconds.
    pub idle_ms: u64,
    pub user_active: bool,
    pub cpu_01: f32,
    pub mem_01: f32,
    pub audio_level_01: f32,
    pub media_playing: bool,
    pub fullscreen_app_active: bool,
    pub monitor_count: u32,
}
