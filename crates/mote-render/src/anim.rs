//! Animation controller: turns simulation state into per-frame [`Pose`]s.
//!
//! The juicy bits live here: a squash-and-stretch spring, blink/saccade
//! timers, walk-cycle phase, dance bounce and landing impulses. All time
//! flows through `update(dt, ...)` with a deterministic internal RNG, so a
//! fixed update sequence always yields identical frames (tested).

use mote_core::BehaviourState;

use crate::creature::{Mouth, Pose};

/// What the animator needs to know each tick.
#[derive(Debug, Clone)]
pub struct AnimInput {
    pub state: BehaviourState,
    pub species: mote_core::SpeciesId,
    pub horizontal_speed: f32,
    pub vel_y: f32,
    pub facing: i8,
    /// Desired gaze in screen space, -1..=1 (e.g. toward cursor).
    pub gaze: (f32, f32),
    /// Smoothed audio peak 0..=1.
    pub audio_level: f32,
    /// External blush request (petting), 0..=1. Decays internally.
    pub blush_push: f32,
    /// Landing impulse 0..=1 set on touchdown (scales with impact).
    pub land_impulse: f32,
    /// Dampen bounce/tilt/bob for motion-sensitive users.
    pub reduce_motion: bool,
}

impl Default for AnimInput {
    fn default() -> Self {
        Self {
            state: BehaviourState::Idle,
            species: mote_core::SpeciesId::Peeker,
            horizontal_speed: 0.0,
            vel_y: 0.0,
            facing: 1,
            gaze: (0.0, 0.0),
            audio_level: 0.0,
            blush_push: 0.0,
            land_impulse: 0.0,
            reduce_motion: false,
        }
    }
}

pub struct Animator {
    time_s: f64,
    rng: u64,
    // Squash spring state + velocity.
    sq: (f32, f32),
    sq_v: (f32, f32),
    blink_open: f32, // 1 = open ... multiplied into eyelid
    next_blink_in: f64,
    blink_age: f64,
    look: (f32, f32),
    next_saccade_in: f64,
    step_phase: f64,
    bounce_phase: f64,
    ear_phase: f64,
    climb_phase: f64,
    startle_age: f64,
    last_state: BehaviourState,
    sleep_amt: f32,
    blush: f32,
    radius: f32,
}

impl Animator {
    pub fn new(radius: f32) -> Self {
        Self {
            time_s: 0.0,
            rng: 0x1234_5678_9ABC_DEF1,
            sq: (1.0, 1.0),
            sq_v: (0.0, 0.0),
            blink_open: 1.0,
            next_blink_in: 2.0,
            blink_age: 99.0,
            look: (0.0, 0.0),
            next_saccade_in: 0.4,
            step_phase: 0.0,
            bounce_phase: 0.0,
            ear_phase: 0.0,
            climb_phase: 0.0,
            startle_age: 99.0,
            last_state: BehaviourState::Idle,
            sleep_amt: 0.0,
            blush: 0.0,
            radius,
        }
    }

    pub fn set_radius(&mut self, r: f32) {
        self.radius = r.clamp(24.0, 160.0);
    }

    fn rand01(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        ((x.wrapping_mul(0x2545F4914F6CDD1D) >> 33) as f64 / u32::MAX as f64) as f32
    }

    pub fn update(&mut self, mut dt: f32, inp: &AnimInput) {
        dt = dt.clamp(0.0005, 0.05);
        self.time_s += dt as f64;
        let t = self.time_s as f32;

        if inp.state != self.last_state {
            // State entry impulses.
            match inp.state {
                BehaviourState::Startled => self.startle_age = 0.0,
                BehaviourState::Landing | BehaviourState::Wobble => {
                    self.kick_squash(0.35 + inp.land_impulse * 0.65);
                }
                _ => {}
            }
            self.last_state = inp.state;
        }
        self.startle_age += dt as f64;
        if inp.land_impulse > 0.01
            && matches!(inp.state, BehaviourState::Landing | BehaviourState::Falling)
        {
            self.kick_squash(inp.land_impulse);
        }

        // --- Gaze: saccade toward the desired target in steps.
        self.next_saccade_in -= dt as f64;
        let (want_x, want_y) = self.gaze_target(inp);
        if self.next_saccade_in <= 0.0 {
            self.next_saccade_in = 0.35 + self.rand01() as f64 * 0.8;
            // Saccade: jump most of the way, keep a little lag for life.
            self.look.0 += (want_x - self.look.0) * (0.55 + self.rand01() * 0.4);
            self.look.1 += (want_y - self.look.1) * (0.55 + self.rand01() * 0.4);
        } else {
            // Smooth micro-drift between saccades.
            self.look.0 += (want_x - self.look.0) * dt * 3.0;
            self.look.1 += (want_y - self.look.1) * dt * 3.0;
        }

        // --- Blink (suppressed asleep / wide-eyed).
        let suppress = matches!(inp.state, BehaviourState::Sleep)
            || matches!(inp.state, BehaviourState::Startled);
        self.blink_age += dt as f64;
        if !suppress {
            self.next_blink_in -= dt as f64;
            if self.next_blink_in <= 0.0 {
                self.next_blink_in = 2.2 + self.rand01() as f64 * 2.8;
                self.blink_age = 0.0;
            }
        }
        // Blink profile: shut over 60ms, open over 120ms.
        let ba = self.blink_age as f32;
        self.blink_open = if suppress && matches!(inp.state, BehaviourState::Sleep) {
            0.0
        } else if ba < 0.06 {
            1.0 - ba / 0.06
        } else if ba < 0.18 {
            (ba - 0.06) / 0.12
        } else {
            1.0
        }
        .clamp(0.0, 1.0);

        // --- Locomotion phase.
        let moving = inp.horizontal_speed > 12.0
            && matches!(
                inp.state,
                BehaviourState::Walk
                    | BehaviourState::Run
                    | BehaviourState::ChaseCursor
                    | BehaviourState::AvoidCursor
                    | BehaviourState::TravelMonitor
            );
        if moving {
            self.step_phase += dt as f64 * (5.0 + inp.horizontal_speed * 0.045) as f64;
        }
        if inp.state == BehaviourState::Climbing {
            self.climb_phase += dt as f64 * 8.0;
        }
        self.bounce_phase += dt as f64
            * if matches!(inp.state, BehaviourState::Dance) {
                2.0 + inp.audio_level as f64 * 1.2
            } else {
                0.0
            };
        self.ear_phase += dt as f64
            * match inp.state {
                BehaviourState::Dance => 9.0,
                BehaviourState::Curious | BehaviourState::ExamineWindow => 5.0,
                BehaviourState::Sleep => 0.8,
                _ => 1.6,
            };

        // --- Squash spring toward state target.
        let (tx, ty) = self.squash_target(inp);
        let k = 170.0;
        let c = 15.0;
        for i in 0..2 {
            let (x, v, tgt) = if i == 0 {
                (self.sq.0, self.sq_v.0, tx)
            } else {
                (self.sq.1, self.sq_v.1, ty)
            };
            let a = (tgt - x) * k - v * c;
            let nv = v + a * dt;
            let nx = x + nv * dt;
            if i == 0 {
                self.sq.0 = nx;
                self.sq_v.0 = nv;
            } else {
                self.sq.1 = nx;
                self.sq_v.1 = nv;
            }
        }

        // --- Sleep + blush envelopes.
        let sleep_tgt = if matches!(inp.state, BehaviourState::Sleep) {
            1.0
        } else {
            0.0
        };
        self.sleep_amt += (sleep_tgt - self.sleep_amt) * (dt * 2.5).min(1.0);
        self.blush = (self.blush + inp.blush_push).min(1.0);
        let blush_tgt = match inp.state {
            BehaviourState::Dance => 0.7,
            BehaviourState::Curious => 0.25,
            _ => 0.0,
        };
        self.blush += (blush_tgt - self.blush) * (dt * 1.5).min(1.0);

        let _ = t;
    }

    fn kick_squash(&mut self, amount: f32) {
        // Landing: push wide + flat; the spring resolves it into a bounce.
        self.sq_v.0 += 6.0 * amount;
        self.sq_v.1 -= 7.5 * amount;
    }

    fn gaze_target(&self, inp: &AnimInput) -> (f32, f32) {
        let (gx, gy) = inp.gaze;
        match inp.state {
            BehaviourState::Curious
            | BehaviourState::LookAround
            | BehaviourState::ExamineWindow
            | BehaviourState::ChaseCursor => (gx.clamp(-1.0, 1.0), gy.clamp(-1.0, 1.0)),
            BehaviourState::Walk | BehaviourState::Run | BehaviourState::TravelMonitor => {
                (inp.facing as f32 * 0.55, 0.25)
            }
            BehaviourState::Dance => (self.look.0 * 0.5, -0.4),
            BehaviourState::Sleep => (0.0, 0.5),
            BehaviourState::Peeking => (self.look.0.clamp(-0.8, 0.8), -0.4),
            BehaviourState::Climbing => (inp.facing as f32 * 0.6, -0.3),
            BehaviourState::Startled | BehaviourState::AvoidCursor => {
                (gx.clamp(-1.0, 1.0), gy.clamp(-1.0, 1.0))
            }
            _ => (inp.facing as f32 * 0.25, 0.1),
        }
    }

    fn squash_target(&self, inp: &AnimInput) -> (f32, f32) {
        match inp.state {
            BehaviourState::Sit => (1.10, 0.88),
            BehaviourState::Sleep => (1.24, 0.74),
            BehaviourState::Stretch | BehaviourState::Waking => (0.90, 1.12),
            BehaviourState::Jumping => (0.88, 1.18),
            BehaviourState::Climbing => (0.85, 1.16),
            BehaviourState::Peeking => (1.06, 0.88),
            BehaviourState::Falling => (0.92, 1.20),
            BehaviourState::Startled => (1.10, 1.10),
            BehaviourState::Dance => {
                let beat = (self.bounce_phase * std::f64::consts::TAU).sin() as f32;
                (
                    1.0 + beat * 0.05 * (0.4 + inp.audio_level),
                    1.0 - beat * 0.05 * (0.4 + inp.audio_level),
                )
            }
            BehaviourState::ReactLoad => (1.16, 0.82),
            BehaviourState::Dragged => (1.06, 0.94),
            _ => (1.0, 1.0),
        }
    }

    /// Build the current pose (call after `update`).
    pub fn pose(&self, inp: &AnimInput) -> Pose {
        let t = self.time_s as f32;
        let moving = inp.horizontal_speed > 12.0
            && matches!(
                inp.state,
                BehaviourState::Walk
                    | BehaviourState::Run
                    | BehaviourState::ChaseCursor
                    | BehaviourState::AvoidCursor
                    | BehaviourState::TravelMonitor
            );
        let dancing = matches!(inp.state, BehaviourState::Dance);
        let is_peeking = matches!(inp.state, BehaviourState::Peeking);
        let is_climbing = matches!(inp.state, BehaviourState::Climbing);
        let is_napping = matches!(inp.state, BehaviourState::Sleep);

        // Breathe: slow idle swell, deeper when sitting, slowest asleep.
        let breathe_f = match inp.state {
            BehaviourState::Sleep => 0.9,
            BehaviourState::Sit => 1.6,
            BehaviourState::Dance => 3.0,
            _ => 2.2,
        };
        let breathe_a = match inp.state {
            BehaviourState::Sleep => 0.030,
            BehaviourState::Sit => 0.022,
            _ => 0.014,
        };
        let breathe = (t * breathe_f * std::f32::consts::TAU * 0.28).sin() * breathe_a;

        // Hop: walk bob or dance bounce, or peeking drop, or napping settling.
        let mut hop = 0.0;
        let mut tilt = 0.0;
        let mut step_phase = None;
        let mut step_amp = 0.0;
        let motion_scale = if inp.reduce_motion { 0.35 } else { 1.0 };
        if moving {
            let ph = self.step_phase;
            step_phase = Some(ph as f32);
            step_amp = (inp.horizontal_speed / 260.0).clamp(0.35, 1.0);
            hop = -((ph * 2.0).sin() as f32).abs() * 3.2 * step_amp * motion_scale;
            tilt = inp.facing as f32 * 0.07 * step_amp * motion_scale;
        }
        if dancing {
            let b = (self.bounce_phase * std::f64::consts::TAU).sin() as f32;
            let b2 = (self.bounce_phase * std::f64::consts::TAU * 2.0).sin() as f32;
            hop = -b.abs() * 8.0 * (0.35 + inp.audio_level) * motion_scale;
            tilt = b2 * 0.16 * motion_scale;
        }
        if is_peeking {
            hop = 40.0;
            tilt = 0.0;
        } else if is_climbing {
            tilt = if inp.facing > 0 { -0.22 } else { 0.22 };
        }
        // Startle shake.
        if matches!(inp.state, BehaviourState::Startled) && self.startle_age < 0.6 {
            tilt += (self.startle_age as f32 * 70.0).sin()
                * 0.07
                * (1.0 - self.startle_age as f32 / 0.6);
        }
        // Load tremble.
        if matches!(inp.state, BehaviourState::ReactLoad) {
            tilt += (t * 31.0).sin() * 0.02;
        }
        // Wobble oscillation.
        if matches!(inp.state, BehaviourState::Wobble | BehaviourState::Dangling) {
            tilt += (t * 9.0).sin() * 0.12;
        }

        let eye_wide = match inp.state {
            BehaviourState::Startled => 1.0,
            BehaviourState::Falling if inp.vel_y > 500.0 => 0.7,
            BehaviourState::Dragged => 0.5,
            BehaviourState::Curious | BehaviourState::ExamineWindow => 0.25,
            BehaviourState::AvoidCursor => 0.4,
            _ => 0.0,
        };
        let mouth = match inp.state {
            BehaviourState::Dance => Mouth::Smile,
            BehaviourState::Startled | BehaviourState::Dragged => Mouth::Oh,
            BehaviourState::Falling if inp.vel_y > 500.0 => Mouth::Oh,
            BehaviourState::ReactLoad => Mouth::Wavy,
            BehaviourState::Sleep => Mouth::Hidden,
            BehaviourState::Sit => Mouth::Smile,
            _ => Mouth::Small,
        };
        let lean = match inp.state {
            BehaviourState::Curious | BehaviourState::ExamineWindow => {
                self.look.0.clamp(-1.0, 1.0) * 0.8
            }
            BehaviourState::AvoidCursor => -self.look.0.clamp(-1.0, 1.0) * 0.7,
            _ => 0.0,
        };
        let eyelid = if is_peeking {
            0.0
        } else {
            1.0 - self.blink_open
        };

        Pose {
            species: inp.species,
            squash_x: (self.sq.0 + breathe * 0.6).clamp(0.6, 1.5),
            squash_y: (self.sq.1 - breathe).clamp(0.55, 1.45),
            tilt,
            hop_px: hop,
            look_x: self.look.0,
            look_y: self.look.1,
            eyelid,
            eye_wide,
            mouth,
            lean,
            step_phase,
            step_amp,
            ear_phase: if inp.reduce_motion {
                0.0
            } else {
                self.ear_phase as f32
            },
            sleep_amount: self.sleep_amt,
            blush: self.blush,
            wilt: if matches!(inp.state, BehaviourState::ReactLoad) {
                1.0
            } else {
                0.0
            },
            radius: self.radius,
            time_s: t,
            is_peeking,
            is_climbing,
            climb_phase: self.climb_phase as f32,
            is_napping,
            facing: inp.facing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(inp: &AnimInput, n: usize, dt: f32) -> Animator {
        let mut a = Animator::new(64.0);
        for _ in 0..n {
            a.update(dt, inp);
        }
        a
    }

    #[test]
    fn deterministic_frames() {
        let inp = AnimInput {
            state: BehaviourState::Walk,
            horizontal_speed: 95.0,
            ..AnimInput::default()
        };
        let a = step(&inp, 120, 1.0 / 60.0);
        let b = step(&inp, 120, 1.0 / 60.0);
        let pa = a.pose(&inp);
        let pb = b.pose(&inp);
        assert!((pa.squash_x - pb.squash_x).abs() < 1e-6);
        assert_eq!(
            crate::creature::draw_mote(&pa),
            crate::creature::draw_mote(&pb)
        );
    }

    #[test]
    fn landing_kick_flattens_then_recovers() {
        let inp = AnimInput {
            state: BehaviourState::Landing,
            land_impulse: 1.0,
            ..AnimInput::default()
        };
        let mut a = Animator::new(64.0);
        a.update(1.0 / 60.0, &inp);
        let flat = a.pose(&inp).squash_y;
        for _ in 0..120 {
            a.update(1.0 / 60.0, &AnimInput::default());
        }
        let rest = a.pose(&AnimInput::default()).squash_y;
        assert!(
            flat < rest,
            "landing should flatten first ({flat} vs {rest})"
        );
        assert!((rest - 1.0).abs() < 0.05, "spring should recover ({rest})");
    }

    #[test]
    fn sleep_closes_eyes() {
        let inp = AnimInput {
            state: BehaviourState::Sleep,
            ..AnimInput::default()
        };
        let a = step(&inp, 300, 1.0 / 60.0);
        let p = a.pose(&inp);
        assert!(p.eyelid > 0.9);
        assert!(p.sleep_amount > 0.9);
    }

    #[test]
    fn all_states_produce_valid_poses() {
        // Every behaviour state must render without panic and stay in range.
        let states = [
            BehaviourState::Idle,
            BehaviourState::LookAround,
            BehaviourState::Walk,
            BehaviourState::Run,
            BehaviourState::Sit,
            BehaviourState::Stretch,
            BehaviourState::Sleep,
            BehaviourState::Waking,
            BehaviourState::Startled,
            BehaviourState::Curious,
            BehaviourState::ChaseCursor,
            BehaviourState::AvoidCursor,
            BehaviourState::Dance,
            BehaviourState::ReactLoad,
            BehaviourState::ExamineWindow,
            BehaviourState::TravelMonitor,
            BehaviourState::Dangling,
            BehaviourState::Wobble,
            BehaviourState::Climbing,
            BehaviourState::Jumping,
            BehaviourState::Falling,
            BehaviourState::Landing,
            BehaviourState::Dragged,
        ];
        for s in states {
            let inp = AnimInput {
                state: s,
                horizontal_speed: 120.0,
                vel_y: 800.0,
                audio_level: 0.6,
                gaze: (0.7, -0.3),
                blush_push: 0.0,
                land_impulse: 0.8,
                species: mote_core::SpeciesId::default(),
                reduce_motion: false,
                facing: -1,
            };
            let mut a = Animator::new(64.0);
            for _ in 0..30 {
                a.update(1.0 / 60.0, &inp);
            }
            let p = a.pose(&inp);
            assert!((0.6..=1.5).contains(&p.squash_x), "{s:?} sx {}", p.squash_x);
            assert!(
                (0.55..=1.45).contains(&p.squash_y),
                "{s:?} sy {}",
                p.squash_y
            );
            let buf = crate::creature::draw_mote(&p);
            assert_eq!(buf.len(), 256 * 256 * 4);
        }
    }
}
