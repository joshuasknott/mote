//! Deterministic animation controller for realistic raster poses.
//!
//! Atlas artwork carries the animal's anatomy. This controller only supplies
//! restrained physical motion: a small breath, grounded step phase, landing
//! impulse, and state transition age. It deliberately does not pretend to
//! animate eyes, mouths, ears, or painted facial details that are not present
//! in the raster frames.

use mote_core::{BehaviourState, SpeciesId};

use crate::creature::Pose;

/// What the animator needs to know each tick.
#[derive(Debug, Clone)]
pub struct AnimInput {
    pub state: BehaviourState,
    pub species: SpeciesId,
    pub horizontal_speed: f32,
    pub vel_y: f32,
    pub facing: i8,
    /// Retained as a stable app input; atlas animals use their authored gaze.
    pub gaze: (f32, f32),
    pub audio_level: f32,
    /// A small physical pet reaction, rather than a painted blush overlay.
    pub blush_push: f32,
    pub land_impulse: f32,
    pub reduce_motion: bool,
}

impl Default for AnimInput {
    fn default() -> Self {
        Self {
            state: BehaviourState::Idle,
            species: SpeciesId::Cat,
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
    sq: (f32, f32),
    sq_v: (f32, f32),
    step_phase: f64,
    bounce_phase: f64,
    climb_phase: f64,
    startle_age: f64,
    last_state: BehaviourState,
    state_age: f32,
    sleep_amt: f32,
    pet_reaction: f32,
    radius: f32,
}

impl Animator {
    pub fn new(radius: f32) -> Self {
        Self {
            time_s: 0.0,
            sq: (1.0, 1.0),
            sq_v: (0.0, 0.0),
            step_phase: 0.0,
            bounce_phase: 0.0,
            climb_phase: 0.0,
            startle_age: 99.0,
            last_state: BehaviourState::Idle,
            state_age: 0.0,
            sleep_amt: 0.0,
            pet_reaction: 0.0,
            radius,
        }
    }

    pub fn set_radius(&mut self, r: f32) {
        self.radius = r.clamp(24.0, 160.0);
    }

    pub fn update(&mut self, mut dt: f32, inp: &AnimInput) {
        dt = dt.clamp(0.0005, 0.05);
        self.time_s += dt as f64;
        if inp.state != self.last_state {
            self.state_age = 0.0;
            if inp.state == BehaviourState::Startled {
                self.startle_age = 0.0;
            }
            self.last_state = inp.state;
        }
        self.state_age += dt;
        self.startle_age += dt as f64;

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
        if inp.state == BehaviourState::Dance {
            self.bounce_phase += dt as f64 * (2.0 + inp.audio_level as f64 * 1.2);
        }

        if inp.species != SpeciesId::Tortoise
            && inp.land_impulse > 0.01
            && matches!(inp.state, BehaviourState::Landing | BehaviourState::Falling)
        {
            self.kick_squash(inp.land_impulse);
        }
        let target_sleep = if inp.state == BehaviourState::Sleep {
            1.0
        } else {
            0.0
        };
        self.sleep_amt += (target_sleep - self.sleep_amt) * (dt * 2.5).min(1.0);
        self.pet_reaction = (self.pet_reaction + inp.blush_push.clamp(0.0, 1.0)).min(1.0);
        self.pet_reaction = (self.pet_reaction - dt * 3.5).max(0.0);

        // Realistic photographs need only a small, recoverable impact bend.
        // Tortoises stay rigid; their slow motion comes from simulation.
        let target = (1.0, 1.0);
        let k = 170.0;
        let c = 18.0;
        for i in 0..2 {
            let (x, v) = if i == 0 {
                (self.sq.0, self.sq_v.0)
            } else {
                (self.sq.1, self.sq_v.1)
            };
            let tgt = if i == 0 { target.0 } else { target.1 };
            let nv = v + ((tgt - x) * k - v * c) * dt;
            let nx = x + nv * dt;
            if i == 0 {
                self.sq = (nx, self.sq.1);
                self.sq_v = (nv, self.sq_v.1);
            } else {
                self.sq = (self.sq.0, nx);
                self.sq_v = (self.sq_v.0, nv);
            }
        }
    }

    fn kick_squash(&mut self, amount: f32) {
        self.sq_v.0 += 2.4 * amount;
        self.sq_v.1 -= 3.0 * amount;
    }

    /// Build the current atlas pose. Call after [`Self::update`].
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
        let motion_scale = if inp.reduce_motion { 0.0 } else { 1.0 };
        let flex_allowed = inp.species != SpeciesId::Tortoise;
        let breath = if flex_allowed && !inp.reduce_motion {
            let amp = if inp.state == BehaviourState::Sleep {
                0.028
            } else {
                0.020
            };
            (t * std::f32::consts::TAU * 0.28).sin() * amp
        } else {
            0.0
        };
        let mut hop = 0.0;
        let mut tilt = 0.0;
        let step_phase = if moving {
            Some(self.step_phase as f32)
        } else {
            None
        };
        if moving {
            let amp = (inp.horizontal_speed / 260.0).clamp(0.35, 1.0);
            hop = -((self.step_phase * 2.0).sin() as f32).abs() * 2.2 * amp * motion_scale;
            tilt = inp.facing as f32 * 0.045 * amp * motion_scale;
        }
        if inp.state == BehaviourState::Dance {
            let beat = (self.bounce_phase * std::f64::consts::TAU).sin() as f32;
            hop = -beat.abs() * 4.0 * (0.35 + inp.audio_level) * motion_scale;
            tilt = (self.bounce_phase * std::f64::consts::TAU * 2.0).sin() as f32
                * 0.07
                * motion_scale;
        }
        if inp.state == BehaviourState::Peeking {
            hop = 40.0 * motion_scale;
        }
        if inp.state == BehaviourState::Climbing {
            tilt = 0.0;
        }
        if inp.state == BehaviourState::Startled && self.startle_age < 0.6 && motion_scale > 0.0 {
            tilt += (self.startle_age as f32 * 70.0).sin()
                * 0.04
                * (1.0 - self.startle_age as f32 / 0.6);
        }
        if matches!(inp.state, BehaviourState::Wobble | BehaviourState::Dangling)
            && motion_scale > 0.0
        {
            tilt += (t * 9.0).sin() * 0.06;
        }
        let pet = if inp.reduce_motion {
            0.0
        } else {
            self.pet_reaction
        };
        hop += -2.2 * pet;
        let lean = inp.facing as f32 * 0.08 * pet;
        Pose {
            species: inp.species,
            state: inp.state,
            state_age: self.state_age,
            squash_x: (self.sq.0 + breath * 0.55).clamp(0.92, 1.08),
            squash_y: (self.sq.1 - breath).clamp(0.92, 1.08),
            tilt,
            hop_px: hop,
            lean,
            step_phase,
            sleep_amount: self.sleep_amt,
            radius: self.radius,
            time_s: t,
            is_peeking: inp.state == BehaviourState::Peeking,
            is_climbing: inp.state == BehaviourState::Climbing,
            climb_phase: self.climb_phase as f32,
            is_napping: inp.state == BehaviourState::Sleep,
            facing: inp.facing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stepped(inp: &AnimInput, n: usize) -> Animator {
        let mut a = Animator::new(64.0);
        for _ in 0..n {
            a.update(1.0 / 60.0, inp);
        }
        a
    }

    #[test]
    fn deterministic_frames() {
        let inp = AnimInput {
            state: BehaviourState::Walk,
            horizontal_speed: 95.0,
            ..Default::default()
        };
        let a = stepped(&inp, 120);
        let b = stepped(&inp, 120);
        assert_eq!(a.pose(&inp).squash_x, b.pose(&inp).squash_x);
        assert_eq!(
            crate::creature::draw_mote(&a.pose(&inp)),
            crate::creature::draw_mote(&b.pose(&inp))
        );
    }

    #[test]
    fn impact_is_small_and_recovers() {
        let inp = AnimInput {
            state: BehaviourState::Landing,
            land_impulse: 1.0,
            ..Default::default()
        };
        let mut a = Animator::new(64.0);
        a.update(1.0 / 60.0, &inp);
        let p = a.pose(&inp);
        assert!(p.squash_y >= 0.92 && p.squash_y <= 1.08);
        for _ in 0..120 {
            a.update(1.0 / 60.0, &AnimInput::default());
        }
        assert!((a.pose(&AnimInput::default()).squash_y - 1.0).abs() < 0.02);
    }

    #[test]
    fn reduced_motion_and_tortoise_are_rigid() {
        let inp = AnimInput {
            species: SpeciesId::Tortoise,
            state: BehaviourState::Dance,
            audio_level: 1.0,
            ..Default::default()
        };
        let a = stepped(&inp, 60);
        let p = a.pose(&inp);
        assert_eq!(p.squash_x, 1.0);
        assert_eq!(p.squash_y, 1.0);
        let quiet = AnimInput {
            reduce_motion: true,
            ..inp
        };
        let q = stepped(&quiet, 60).pose(&quiet);
        assert_eq!(q.tilt, 0.0);
        assert_eq!(q.hop_px, 0.0);
    }

    #[test]
    fn petting_has_small_physical_reaction() {
        let inp = AnimInput {
            blush_push: 1.0,
            ..Default::default()
        };
        let mut a = Animator::new(64.0);
        a.update(1.0 / 60.0, &inp);
        let p = a.pose(&inp);
        assert!(p.hop_px < 0.0 && p.hop_px > -3.0);
        assert!(p.lean.abs() <= 0.08);
    }
}
