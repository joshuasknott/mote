//! Behaviour system: a state machine with utility-weighted transitions.
//!
//! Design goals (from the product brief):
//! - coherent, not flappy: every state has a minimum duration and most
//!   transitions have cooldowns;
//! - personality/mood/environment/recent-actions shape decisions, not dice;
//! - all randomness flows through a deterministic xorshift stream owned by
//!   the [`Brain`], so replays with the same inputs reproduce.
//!
//! The brain outputs an [`Intent`] each tick; `lib.rs` translates intents
//! into locomotion. Rendering-only states (blink, breathe) live in
//! `mote-render` and are deliberately NOT behaviour states.

use crate::personality::{Drives, Personality};
use crate::physics::Body;
use crate::species::SpeciesId;
use crate::world::{SupportKind, WorldSnapshot};
use crate::SenseInput;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BehaviourState {
    Idle,
    LookAround,
    Walk,
    Run,
    Sit,
    Stretch,
    Sleep,
    Waking,
    Startled,
    Curious,
    ChaseCursor,
    AvoidCursor,
    Dance,
    ReactLoad,
    ExamineWindow,
    TravelMonitor,
    Dangling,
    Wobble,
    Climbing,
    Peeking,
    Jumping,
    Falling,
    Landing,
    Dragged,
}

impl BehaviourState {
    /// Minimum time in state before any transition, milliseconds.
    pub fn min_duration_ms(&self) -> u64 {
        match self {
            BehaviourState::Idle => 1200,
            BehaviourState::LookAround => 1500,
            BehaviourState::Walk => 1800,
            BehaviourState::Run => 1200,
            BehaviourState::Sit => 3500,
            BehaviourState::Stretch => 1800,
            BehaviourState::Sleep => 8000,
            BehaviourState::Waking => 1600,
            BehaviourState::Startled => 700,
            BehaviourState::Curious => 2200,
            BehaviourState::ChaseCursor => 2500,
            BehaviourState::AvoidCursor => 1500,
            BehaviourState::Dance => 4000,
            BehaviourState::ReactLoad => 2500,
            BehaviourState::ExamineWindow => 3000,
            BehaviourState::TravelMonitor => 3000,
            BehaviourState::Dangling => 1200,
            BehaviourState::Wobble => 900,
            BehaviourState::Climbing => 1800,
            BehaviourState::Peeking => 2200,
            BehaviourState::Jumping => 400,
            BehaviourState::Falling => 200,
            BehaviourState::Landing => 500,
            BehaviourState::Dragged => 100,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentKind {
    Stay,
    WalkTo,
    RunTo,
    WanderTo,
    JumpTo,
    ClimbTo,
    Peek,
    ChaseCursor,
    AvoidCursor,
    Sit,
    Sleep,
    LookAt,
    WatchWindow,
    Dance,
    ReactLoad,
    Dragged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub kind: IntentKind,
    pub target_x: f32,
    pub target_y: f32,
    pub target_surface: Option<u64>,
}

impl Intent {
    pub fn stay() -> Self {
        Self {
            kind: IntentKind::Stay,
            target_x: f32::NAN,
            target_y: f32::NAN,
            target_surface: None,
        }
    }
    pub fn walk_to(x: f32) -> Self {
        Self {
            kind: IntentKind::WalkTo,
            target_x: x,
            target_y: f32::NAN,
            target_surface: None,
        }
    }
    pub fn sleep() -> Self {
        Self {
            kind: IntentKind::Sleep,
            target_x: f32::NAN,
            target_y: f32::NAN,
            target_surface: None,
        }
    }
}

/// Read-only snapshot the brain uses to score options.
#[derive(Debug, Clone)]
pub struct DecisionContext {
    pub now_ms: u64,
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub cursor_speed: f32,
    pub cursor_dist: f32,
    pub cursor_approaching_fast: bool,
    pub idle_s: f32,
    pub user_active: bool,
    pub cpu_01: f32,
    pub audio_level: f32,
    pub media_playing: bool,
    pub fullscreen: bool,
    pub grounded: bool,
    pub near_edge: bool,
    pub edge_side: i8,
    /// Species-specific movement constraints used when scoring behaviours.
    pub species: SpeciesId,
    pub personality: Personality,
}

impl DecisionContext {
    pub fn from_sense(s: &SenseInput, body: &Body, _drives: &Drives, p: &Personality) -> Self {
        Self::from_sense_for_species(s, body, _drives, SpeciesId::default(), p)
    }

    pub fn from_sense_for_species(
        s: &SenseInput,
        body: &Body,
        _drives: &Drives,
        species: SpeciesId,
        p: &Personality,
    ) -> Self {
        let dx = s.cursor_x - body.pos.x;
        let dy = s.cursor_y - body.pos.y;
        let dist = (dx * dx + dy * dy).sqrt();
        // Approaching fast = cursor moving quickly AND getting closer.
        // Approximate: high speed + within 260px.
        let approaching = s.cursor_speed_px_s > 900.0 && dist < 300.0;
        Self {
            now_ms: 0,
            cursor_x: s.cursor_x,
            cursor_y: s.cursor_y,
            cursor_speed: s.cursor_speed_px_s,
            cursor_dist: dist,
            cursor_approaching_fast: approaching,
            idle_s: s.idle_ms as f32 / 1000.0,
            user_active: s.user_active,
            cpu_01: s.cpu_01,
            audio_level: s.audio_level_01,
            media_playing: s.media_playing,
            fullscreen: s.fullscreen_app_active,
            grounded: body.grounded(),
            near_edge: false,
            edge_side: 0,
            species,
            personality: *p,
        }
    }
}

/// The behaviour brain. Owns state, cooldowns and a deterministic RNG.
#[derive(Debug, Clone)]
pub struct Brain {
    pub state: BehaviourState,
    pub state_enter_ms: u64,
    pub cooldowns: HashMap<BehaviourState, u64>,
    pub rng_state: u64,
    pub current_target_x: Option<f32>,
    pub current_target_surface: Option<u64>,
    pub last_sleep_ms: u64,
    pub last_jump_ms: u64,
    pub last_chase_ms: u64,
    /// Consecutive decisions spent stationary (Sit/Idle/LookAround). Creates
    /// restlessness pressure so Mote can never statue-sit forever.
    pub still_streak: u32,
    pub forced_state: Option<BehaviourState>,
    /// Set by the app for one-shot commands (sleep now, come here...).
    pub command: Option<AppCommand>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppCommand {
    SleepNow,
    WakeNow,
    ComeHere { x: f32, y: f32 },
    Hide,
}

impl Brain {
    pub fn new(now_ms: u64) -> Self {
        Self {
            state: BehaviourState::Idle,
            state_enter_ms: now_ms,
            cooldowns: HashMap::new(),
            rng_state: 0x9E3779B97F4A7C15 ^ (now_ms.wrapping_mul(0xBF58476D1CE4E5B9)),
            current_target_x: None,
            current_target_surface: None,
            last_sleep_ms: 0,
            last_jump_ms: 0,
            last_chase_ms: 0,
            still_streak: 0,
            forced_state: None,
            command: None,
        }
    }

    pub fn time_in_state(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.state_enter_ms)
    }

    pub fn set_state(&mut self, s: BehaviourState, now_ms: u64) {
        if self.state != s {
            self.state = s;
            self.state_enter_ms = now_ms;
        }
    }

    fn on_cooldown(&self, s: BehaviourState, now_ms: u64) -> bool {
        self.cooldowns.get(&s).map(|t| *t > now_ms).unwrap_or(false)
    }

    fn set_cooldown(&mut self, s: BehaviourState, now_ms: u64, ms: u64) {
        self.cooldowns.insert(s, now_ms + ms);
    }

    /// Deterministic xorshift64*; returns 0..=1.
    fn rand01(&mut self) -> f32 {
        let mut x = self.rng_state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng_state = x;
        ((x.wrapping_mul(0x2545F4914F6CDD1D) >> 33) as f64 / u32::MAX as f64) as f32
    }

    pub fn observe_physics(&mut self, events: &[crate::physics::PhysicsEvent], now_ms: u64) {
        use crate::physics::PhysicsEvent as E;
        for e in events {
            match e {
                E::Landed { impact_px_s, .. } => {
                    if *impact_px_s > 700.0 {
                        self.set_state(BehaviourState::Landing, now_ms);
                    } else if self.state == BehaviourState::Falling
                        || self.state == BehaviourState::Jumping
                    {
                        self.set_state(BehaviourState::Idle, now_ms);
                    }
                }
                E::FellOffEdge { .. } | E::SupportLost { .. } => {
                    if self.state != BehaviourState::Dragged {
                        self.set_state(BehaviourState::Falling, now_ms);
                    }
                }
                E::Recovered { .. } => {
                    self.set_state(BehaviourState::Wobble, now_ms);
                }
                E::HitWall { .. } => {
                    if matches!(self.state, BehaviourState::Walk | BehaviourState::Run) {
                        self.set_state(BehaviourState::LookAround, now_ms);
                    }
                }
            }
        }
    }

    /// Main decision tick. Returns the current intent every call; may change
    /// state as a side effect.
    pub fn update(
        &mut self,
        now_ms: u64,
        _dt: f32,
        ctx: &DecisionContext,
        body: &Body,
        world: &WorldSnapshot,
        drives: &Drives,
    ) -> Intent {
        // 0. Forced / commanded states win immediately.
        if let Some(cmd) = self.command.take() {
            match cmd {
                AppCommand::SleepNow => {
                    self.set_state(BehaviourState::Sleep, now_ms);
                    self.last_sleep_ms = now_ms;
                    return Intent::sleep();
                }
                AppCommand::WakeNow => {
                    self.set_state(BehaviourState::Waking, now_ms);
                    return Intent::stay();
                }
                AppCommand::ComeHere { x, .. } => {
                    self.current_target_x = Some(x);
                    let state = if ctx.species.motion().can_run {
                        BehaviourState::Run
                    } else {
                        BehaviourState::Walk
                    };
                    self.set_state(state, now_ms);
                    return Intent {
                        kind: if ctx.species.motion().can_run {
                            IntentKind::RunTo
                        } else {
                            IntentKind::WalkTo
                        },
                        target_x: x,
                        target_y: f32::NAN,
                        target_surface: None,
                    };
                }
                AppCommand::Hide => {
                    return Intent::stay();
                }
            }
        }
        if let Some(f) = self.forced_state.take() {
            self.set_state(f, now_ms);
        }

        // A command or restored state must not bypass an animal's physical
        // limits. Voluntary climbing/jumping/running is species-gated here,
        // with a quiet walk as the safe fallback.
        let motion = ctx.species.motion();
        if (self.state == BehaviourState::Climbing && !motion.can_climb)
            || (self.state == BehaviourState::Jumping && !motion.can_jump)
            || (matches!(
                self.state,
                BehaviourState::Run | BehaviourState::ChaseCursor
            ) && !motion.can_run)
        {
            self.set_state(BehaviourState::Walk, now_ms);
        }

        // 1. Airborne states are physics-driven; don't re-decide mid-air.
        if !body.grounded() && self.state != BehaviourState::Dragged {
            if self.state != BehaviourState::Falling && self.state != BehaviourState::Jumping {
                self.set_state(BehaviourState::Falling, now_ms);
            }
            return Intent::stay();
        }
        // While dragged, an external hand owns the body: hold the state and
        // never re-decide until release.
        if self.state == BehaviourState::Dragged {
            return Intent {
                kind: IntentKind::Dragged,
                target_x: f32::NAN,
                target_y: f32::NAN,
                target_surface: None,
            };
        }

        // 2. Reactive interrupts (checked every tick, bypass min-duration
        //    only for genuinely urgent things).
        if self.state != BehaviourState::Dragged {
            // Fast cursor approach -> startle (scaled by skittishness;
            // unflappable animals such as tortoises stay calm).
            let startle_dist = 130.0 + ctx.personality.skittishness * 140.0;
            if ctx.cursor_approaching_fast
                && ctx.cursor_dist < startle_dist
                && ctx.personality.skittishness > 0.15
                && !self.on_cooldown(BehaviourState::Startled, now_ms)
            {
                self.set_state(BehaviourState::Startled, now_ms);
                self.set_cooldown(BehaviourState::Startled, now_ms, 6000);
                return Intent::stay();
            }
            // Idle computer -> fall asleep (the headline idle behaviour).
            if ctx.idle_s > 120.0
                && !matches!(self.state, BehaviourState::Sleep | BehaviourState::Waking)
                && now_ms - self.last_sleep_ms > 30_000
            {
                self.set_state(BehaviourState::Sleep, now_ms);
                self.last_sleep_ms = now_ms;
                return Intent::sleep();
            }
            // User returned while asleep -> wake.
            if self.state == BehaviourState::Sleep && ctx.idle_s < 5.0 {
                self.set_state(BehaviourState::Waking, now_ms);
                return Intent::stay();
            }
            // Heavy sustained load -> react (cooldown-gated).
            if ctx.cpu_01 > 0.85
                && !self.on_cooldown(BehaviourState::ReactLoad, now_ms)
                && self.time_in_state(now_ms) > 1500
            {
                self.set_state(BehaviourState::ReactLoad, now_ms);
                self.set_cooldown(BehaviourState::ReactLoad, now_ms, 45_000);
                return Intent {
                    kind: IntentKind::ReactLoad,
                    target_x: f32::NAN,
                    target_y: f32::NAN,
                    target_surface: None,
                };
            }
        }

        // 3. Early completion: Climbing finished by scrambling onto window top ledge.
        if self.state == BehaviourState::Climbing
            && body.grounded()
            && body
                .grounded_surface
                .and_then(|id| world.support(id))
                .map(|s| !s.is_wall())
                .unwrap_or(false)
        {
            self.set_state(BehaviourState::Idle, now_ms);
            self.set_cooldown(BehaviourState::Climbing, now_ms, 25_000);
            return self.intent_for_state(body, world, ctx);
        }

        // If still inside the minimum duration, keep executing the state.
        if self.time_in_state(now_ms) < self.state.min_duration_ms() {
            return self.intent_for_state(body, world, ctx);
        }

        // 4. Otherwise score candidate next states by utility.
        let next = self.choose_next(now_ms, ctx, body, world, drives);
        // Restlessness: each stationary decision deepens the streak;
        // locomotion clears it (applied inside choose_next's weights).
        if matches!(
            next,
            BehaviourState::Sit | BehaviourState::Idle | BehaviourState::LookAround
        ) {
            self.still_streak = self.still_streak.saturating_add(1);
        } else {
            self.still_streak = 0;
        }
        self.set_state(next, now_ms);
        // Entering sleep records it; entering chase records it (cooldowns).
        match next {
            BehaviourState::Sleep => self.last_sleep_ms = now_ms,
            BehaviourState::ChaseCursor => {
                self.last_chase_ms = now_ms;
                self.set_cooldown(BehaviourState::ChaseCursor, now_ms, 25_000);
            }
            BehaviourState::Dance => {
                self.set_cooldown(BehaviourState::Dance, now_ms, 30_000);
            }
            BehaviourState::ExamineWindow => {
                self.set_cooldown(BehaviourState::ExamineWindow, now_ms, 40_000);
            }
            BehaviourState::Peeking => {
                self.set_cooldown(BehaviourState::Peeking, now_ms, 20_000);
            }
            BehaviourState::Climbing => {
                self.set_cooldown(BehaviourState::Climbing, now_ms, 25_000);
            }
            _ => {}
        }
        self.intent_for_state(body, world, ctx)
    }

    /// Utility scoring: each candidate gets a weight from drives + context +
    /// personality; the max wins with a small deterministic jitter so ties
    /// don't always break the same way. All weights >= 0.
    fn choose_next(
        &mut self,
        now_ms: u64,
        ctx: &DecisionContext,
        body: &Body,
        world: &WorldSnapshot,
        drives: &Drives,
    ) -> BehaviourState {
        let motion = ctx.species.motion();
        // Sleeping persists until woken.
        if self.state == BehaviourState::Sleep {
            return BehaviourState::Sleep;
        }
        // Startled resolves into avoid/curious/idle, never straight back.
        if self.state == BehaviourState::Startled {
            let r = self.rand01();
            if ctx.cursor_dist < 130.0 && r < 0.55 {
                return BehaviourState::AvoidCursor;
            } else if r < 0.8 {
                return BehaviourState::Curious;
            } else {
                return BehaviourState::Idle;
            }
        }

        // Personality traits shape the drive dynamics (see personality.rs);
        // per-candidate trait weights are folded into the constants below
        // with a fresh deterministic jitter draw per candidate.

        let mut cands: Vec<(BehaviourState, f32)> = Vec::new();
        // Restlessness pressure: +0.12 per consecutive stationary decision,
        // capped at +1.2. Guarantees eventual movement no matter how the
        // drive weights settle — Mote may ignore you, never forever.
        let restless = (self.still_streak as f32 * 0.12).min(1.2);

        // Sitting / idling is the attractor when energy is low.
        let sit_w = 0.25
            + drives.sleepiness * 0.70
            + (1.0 - drives.energy) * 0.50
            + ctx.personality.laziness * 0.25;
        cands.push((BehaviourState::Sit, sit_w + self.rand01() * 0.15));

        // Wandering scales with boredom + energy (+ boldness + restlessness).
        let walk_w = 0.35
            + drives.boredom * 0.90
            + drives.energy * 0.30
            + ctx.personality.boldness * 0.20
            + restless;
        cands.push((BehaviourState::Walk, walk_w + self.rand01() * 0.15));

        // Looking around: cheap default that breaks up walks.
        cands.push((
            BehaviourState::LookAround,
            0.4 + drives.curiosity * 0.4 + restless * 0.5 + self.rand01() * 0.15,
        ));

        // Stretch after sitting/sleeping.
        if matches!(self.state, BehaviourState::Sit | BehaviourState::Waking) {
            cands.push((BehaviourState::Stretch, 0.9 + self.rand01() * 0.15));
        }

        // Sleep / ledge nap when idle or sleepy or lazy on a window ledge.
        let on_ledge = body.grounded()
            && body
                .grounded_surface
                .and_then(|id| world.support(id))
                .map(|s| s.kind == SupportKind::Window)
                .unwrap_or(false);
        let nap_affinity = ctx.personality.laziness * 0.80 + if on_ledge { 0.70 } else { 0.0 };
        let sleep_w = if ctx.idle_s > 60.0 {
            1.6 + drives.sleepiness + nap_affinity
        } else {
            drives.sleepiness * 0.90 + nap_affinity - 0.20
        };
        if sleep_w > 0.05 && now_ms - self.last_sleep_ms > 20_000 {
            cands.push((BehaviourState::Sleep, sleep_w + self.rand01() * 0.15));
        }

        // Cursor play: chase occasionally when bored + cursor near & slow;
        // avoid when cursor very close and fast handled above, or when idle.
        if motion.can_chase_cursor
            && ctx.user_active
            && !ctx.fullscreen
            && !self.on_cooldown(BehaviourState::ChaseCursor, now_ms)
            && ctx.cursor_dist < 420.0
            && ctx.cursor_speed < 500.0
            && drives.boredom > 0.40
            && drives.energy > 0.30
        {
            let chase_w = 0.35
                + ctx.personality.playfulness * 0.60
                + ctx.personality.boldness * 0.30
                + drives.boredom * 0.60;
            cands.push((BehaviourState::ChaseCursor, chase_w + self.rand01() * 0.15));
        }
        if ctx.cursor_dist < 90.0 && ctx.cursor_speed > 250.0 {
            let avoid_w = 0.35 + ctx.personality.skittishness * 0.65;
            cands.push((BehaviourState::AvoidCursor, avoid_w + self.rand01() * 0.15));
        }
        if ctx.cursor_dist < 500.0 && ctx.cursor_speed < 120.0 && drives.curiosity > 0.4 {
            cands.push((
                BehaviourState::Curious,
                0.35 + drives.curiosity * 0.5 + self.rand01() * 0.15,
            ));
        }

        // Music: dance sometimes while playing, bob otherwise (handled by
        // renderer reading audio level; Dance is the explicit state).
        if ctx.media_playing && drives.energy > 0.25 {
            if !self.on_cooldown(BehaviourState::Dance, now_ms) {
                let dance_w = 0.35 + ctx.personality.music_love * 0.70 + drives.excitement * 0.60;
                cands.push((BehaviourState::Dance, dance_w + self.rand01() * 0.15));
            } else {
                // Bob along without committing to the full dance.
                cands.push((BehaviourState::LookAround, 0.2));
            }
        }

        // Window examination when the user is active and a window is near.
        if ctx.user_active
            && !self.on_cooldown(BehaviourState::ExamineWindow, now_ms)
            && drives.curiosity > 0.45
        {
            cands.push((
                BehaviourState::ExamineWindow,
                0.4 + drives.curiosity * 0.5 + self.rand01() * 0.15,
            ));
        }

        // Window peeking: when standing on a window top and curious/playful.
        let on_window = body.grounded()
            && body
                .grounded_surface
                .and_then(|id| world.support(id))
                .map(|s| s.kind == SupportKind::Window)
                .unwrap_or(false);
        if on_window && !self.on_cooldown(BehaviourState::Peeking, now_ms) {
            let peek_bonus = if ctx.personality.skittishness > 0.60 {
                0.30
            } else {
                0.0
            };
            let peek_w = 0.35
                + ctx.personality.playfulness * 0.35
                + drives.curiosity * 0.55
                + peek_bonus
                + self.rand01() * 0.15;
            cands.push((BehaviourState::Peeking, peek_w));
        }

        // Wall climbing: when near a vertical window wall and bold/curious.
        if motion.can_climb && !self.on_cooldown(BehaviourState::Climbing, now_ms) {
            if let Some(wall) = world.wall_near(body.pos.x, body.pos.y, 60.0) {
                if body.pos.y > wall.y + 20.0 {
                    let climb_w = 0.40
                        + ctx.personality.boldness * 0.90
                        + drives.curiosity * 0.50
                        + self.rand01() * 0.15;
                    cands.push((BehaviourState::Climbing, climb_w));
                }
            }
        }

        // Jumping to another surface when bored and targets exist.
        let has_targets = !world
            .jump_targets(
                (body.pos.x, body.pos.y),
                body.grounded_surface,
                motion.jump_reach_px,
                motion.jump_height_px,
            )
            .is_empty();
        if motion.can_jump
            && has_targets
            && drives.boredom > 0.5
            && drives.energy > 0.4
            && now_ms - self.last_jump_ms > 12_000
        {
            cands.push((
                BehaviourState::Jumping,
                0.45 + drives.boredom * 0.6 + self.rand01() * 0.15,
            ));
        }

        // Pick the max-weight candidate.
        let mut best = BehaviourState::Idle;
        let mut best_w = 0.30 + self.rand01() * 0.15; // idle baseline
        for (s, w) in cands {
            if w > best_w {
                best_w = w;
                best = s;
            }
        }
        if best == BehaviourState::Jumping {
            self.last_jump_ms = now_ms;
        }
        best
    }

    /// Build the locomotion intent for the current state (also used to keep
    /// executing a state inside its minimum duration).
    fn intent_for_state(
        &mut self,
        body: &Body,
        world: &WorldSnapshot,
        ctx: &DecisionContext,
    ) -> Intent {
        match self.state {
            BehaviourState::Idle | BehaviourState::LookAround => Intent::stay(),
            BehaviourState::Sit => Intent {
                kind: IntentKind::Sit,
                target_x: f32::NAN,
                target_y: f32::NAN,
                target_surface: None,
            },
            BehaviourState::Sleep => Intent::sleep(),
            BehaviourState::Waking | BehaviourState::Stretch | BehaviourState::Startled => {
                Intent::stay()
            }
            BehaviourState::Peeking => Intent {
                kind: IntentKind::Peek,
                target_x: body.pos.x,
                target_y: body.pos.y,
                target_surface: body.grounded_surface,
            },
            BehaviourState::Climbing => {
                let (target_x, target_y, target_surf) = if let Some(id) = body.grounded_surface {
                    if let Some(s) = world.support(id) {
                        if s.is_wall() {
                            (s.x1, s.y, Some(s.id))
                        } else {
                            // Already on a horizontal ledge; do not latch back to the wall
                            (body.pos.x, s.y, Some(s.id))
                        }
                    } else {
                        (body.pos.x, body.pos.y - 120.0, None)
                    }
                } else if let Some(wall) = world.wall_near(body.pos.x, body.pos.y, 60.0) {
                    (wall.x1, wall.y, Some(wall.id))
                } else {
                    (body.pos.x, body.pos.y - 120.0, None)
                };
                Intent {
                    kind: IntentKind::ClimbTo,
                    target_x,
                    target_y,
                    target_surface: target_surf,
                }
            }
            BehaviourState::Walk => {
                if let Some(x) = self.current_target_x {
                    if (x - body.pos.x).abs() < 8.0 {
                        self.current_target_x = None;
                        return Intent::stay();
                    }
                    return Intent::walk_to(x);
                }
                // Pick a stroll target on the current support, biased away
                // from edges and toward the middle on small windows.
                let x = self.pick_stroll_target(body, world, false);
                self.current_target_x = Some(x);
                Intent::walk_to(x)
            }
            BehaviourState::Run | BehaviourState::TravelMonitor => {
                if let Some(x) = self.current_target_x {
                    if (x - body.pos.x).abs() < 10.0 {
                        self.current_target_x = None;
                        return Intent::stay();
                    }
                    return Intent {
                        kind: if ctx.species.motion().can_run {
                            IntentKind::RunTo
                        } else {
                            IntentKind::WalkTo
                        },
                        target_x: x,
                        target_y: f32::NAN,
                        target_surface: None,
                    };
                }
                let x = self.pick_stroll_target(body, world, true);
                self.current_target_x = Some(x);
                Intent {
                    kind: if ctx.species.motion().can_run {
                        IntentKind::RunTo
                    } else {
                        IntentKind::WalkTo
                    },
                    target_x: x,
                    target_y: f32::NAN,
                    target_surface: None,
                }
            }
            BehaviourState::ChaseCursor => Intent {
                kind: IntentKind::ChaseCursor,
                target_x: ctx.cursor_x,
                target_y: ctx.cursor_y,
                target_surface: None,
            },
            BehaviourState::AvoidCursor => Intent {
                kind: IntentKind::AvoidCursor,
                target_x: ctx.cursor_x,
                target_y: ctx.cursor_y,
                target_surface: None,
            },
            BehaviourState::Curious => Intent {
                kind: IntentKind::LookAt,
                target_x: ctx.cursor_x,
                target_y: ctx.cursor_y,
                target_surface: None,
            },
            BehaviourState::Dance => Intent {
                kind: IntentKind::Dance,
                target_x: f32::NAN,
                target_y: f32::NAN,
                target_surface: None,
            },
            BehaviourState::ReactLoad => Intent {
                kind: IntentKind::ReactLoad,
                target_x: f32::NAN,
                target_y: f32::NAN,
                target_surface: None,
            },
            BehaviourState::ExamineWindow => Intent {
                kind: IntentKind::WatchWindow,
                target_x: f32::NAN,
                target_y: f32::NAN,
                target_surface: None,
            },
            BehaviourState::Jumping => {
                if !ctx.species.motion().can_jump {
                    self.set_state_time_only(BehaviourState::Walk);
                    return Intent::walk_to(self.pick_stroll_target(body, world, false));
                }
                let targets = world.jump_targets(
                    (body.pos.x, body.pos.y),
                    body.grounded_surface,
                    ctx.species.motion().jump_reach_px,
                    ctx.species.motion().jump_height_px,
                );
                if let Some(t) = targets.into_iter().next() {
                    self.current_target_surface = Some(t.id);
                    Intent {
                        kind: IntentKind::JumpTo,
                        target_x: t.center_x(),
                        target_y: t.y,
                        target_surface: Some(t.id),
                    }
                } else {
                    // No valid target: downgrade to a walk.
                    self.set_state_time_only(BehaviourState::Walk);
                    Intent::walk_to(self.pick_stroll_target(body, world, false))
                }
            }
            BehaviourState::Falling | BehaviourState::Landing | BehaviourState::Wobble => {
                Intent::stay()
            }
            BehaviourState::Dangling => Intent::stay(),
            BehaviourState::Dragged => Intent {
                kind: IntentKind::Dragged,
                target_x: f32::NAN,
                target_y: f32::NAN,
                target_surface: None,
            },
        }
    }

    fn set_state_time_only(&mut self, _s: BehaviourState) {
        // Used when downgrading Jump->Walk without resetting cooldown logic.
        self.state = BehaviourState::Walk;
    }

    /// Pick a stroll target on the current support (or taskbar fallback).
    fn pick_stroll_target(&mut self, body: &Body, world: &WorldSnapshot, far: bool) -> f32 {
        let range = if far { 0.42 } else { 0.30 };
        let r = self.rand01();
        if let Some(id) = body.grounded_surface {
            if let Some(s) = world.support(id) {
                let span = (s.x2 - s.x1) * range;
                // Edge awareness: near an end, head back inward instead of
                // pacing into the wall.
                let margin = 60.0;
                let near_left = body.pos.x - s.x1 < margin;
                let near_right = s.x2 - body.pos.x < margin;
                let dir = if near_left || (!near_right && r >= 0.5) {
                    1.0
                } else {
                    -1.0
                };
                let target = body.pos.x + dir * (span * (0.4 + self.rand01() * 0.6));
                // Keep well inside the support so Mote doesn't constantly
                // walk off edges; edge-testing is covered by physics tests.
                let m = 24.0;
                if s.width() > m * 2.0 + 10.0 {
                    return target.clamp(s.x1 + m, s.x2 - m);
                }
                return s.center_x();
            }
        }
        // Fallback: wander along the taskbar or screen bottom.
        if let Some(t) = world.taskbar() {
            let m = 30.0;
            let target = body.pos.x + if r < 0.5 { -160.0 } else { 160.0 };
            return target.clamp(t.x1 + m, (t.x2 - m).max(t.x1 + m));
        }
        body.pos.x
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::personality::Personality;
    use crate::physics::{Body, SimConfig};
    use crate::world::{MonitorRect, Support, SupportKind, VirtualRect, WorldSnapshot};

    fn test_world() -> WorldSnapshot {
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

    fn idle_ctx() -> DecisionContext {
        DecisionContext {
            now_ms: 0,
            cursor_x: 1500.0,
            cursor_y: 500.0,
            cursor_speed: 0.0,
            cursor_dist: 1000.0,
            cursor_approaching_fast: false,
            idle_s: 1.0,
            user_active: false,
            cpu_01: 0.1,
            audio_level: 0.0,
            media_playing: false,
            fullscreen: false,
            grounded: true,
            near_edge: false,
            edge_side: 0,
            species: crate::species::SpeciesId::Cat,
            personality: Personality::default(),
        }
    }

    #[test]
    fn long_idle_leads_to_sleep() {
        let world = test_world();
        let mut brain = Brain::new(0);
        let mut body = Body::new(500.0, 1040.0);
        body.grounded_surface = Some(1);
        let drives = Drives::default();
        let mut ctx = idle_ctx();
        ctx.idle_s = 200.0;
        let intent = brain.update(60_000, 0.016, &ctx, &body, &world, &drives);
        assert_eq!(brain.state, BehaviourState::Sleep);
        assert_eq!(intent.kind, IntentKind::Sleep);
    }

    #[test]
    fn fast_cursor_startles() {
        let world = test_world();
        let mut brain = Brain::new(0);
        let mut body = Body::new(500.0, 1040.0);
        body.grounded_surface = Some(1);
        let drives = Drives::default();
        let mut ctx = idle_ctx();
        ctx.cursor_approaching_fast = true;
        ctx.cursor_dist = 150.0;
        brain.update(5_000, 0.016, &ctx, &body, &world, &drives);
        assert_eq!(brain.state, BehaviourState::Startled);
        // Cooldown prevents immediate re-trigger.
        brain.update(5_800, 0.016, &ctx, &body, &world, &drives);
        assert_ne!(brain.state, BehaviourState::Startled);
    }

    #[test]
    fn min_duration_honoured() {
        let world = test_world();
        let mut brain = Brain::new(0);
        brain.set_state(BehaviourState::Sit, 10_000);
        let mut body = Body::new(500.0, 1040.0);
        body.grounded_surface = Some(1);
        let drives = Drives::default();
        let ctx = idle_ctx();
        brain.update(10_500, 0.016, &ctx, &body, &world, &drives);
        assert_eq!(brain.state, BehaviourState::Sit);
    }

    #[test]
    fn no_flapping_over_time() {
        // Run 3 simulated minutes of idle decisions; count transitions.
        let world = test_world();
        let mut brain = Brain::new(0);
        let body = Body::new(500.0, 1040.0);
        let drives = Drives::default();
        let ctx = idle_ctx();
        let mut transitions = 0;
        let mut last = brain.state;
        let mut t = 0u64;
        while t < 180_000 {
            t += 500;
            brain.update(t, 0.5, &ctx, &body, &world, &drives);
            if brain.state != last {
                transitions += 1;
                last = brain.state;
            }
        }
        // Coherent behaviour: far fewer than one transition per 2s.
        assert!(transitions < 60, "too flappy: {transitions} transitions");
    }

    #[test]
    fn deterministic_with_same_seed() {
        let world = test_world();
        let body = Body::new(500.0, 1040.0);
        let drives = Drives::default();
        let ctx = idle_ctx();
        let run = || {
            let mut b = Brain::new(42);
            let mut seq = Vec::new();
            let mut t = 0u64;
            while t < 60_000 {
                t += 500;
                b.update(t, 0.5, &ctx, &body, &world, &drives);
                seq.push(b.state as u8);
            }
            seq
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn dragged_holds_without_flapping() {
        let world = test_world();
        let mut brain = Brain::new(0);
        brain.forced_state = Some(BehaviourState::Dragged);
        // Airborne body (being carried): must stay Dragged across ticks.
        let mut body = Body::new(500.0, 800.0);
        let drives = Drives::default();
        let ctx = idle_ctx();
        for t in (0..5000).step_by(500) {
            let intent = brain.update(t as u64 + 60_000, 0.5, &ctx, &body, &world, &drives);
            assert_eq!(brain.state, BehaviourState::Dragged);
            assert_eq!(intent.kind, IntentKind::Dragged);
            body.grounded_surface = None;
        }
    }

    #[test]
    fn restlessness_breaks_sit_lock() {
        // Drives tuned to the Sit-lock trap: low energy, middling
        // sleepiness, zero boredom, active user. Without restlessness this
        // sits forever; with it, locomotion must appear.
        let world = test_world();
        let mut brain = Brain::new(0);
        let body = Body {
            grounded_surface: Some(1),
            ..Body::new(500.0, 1040.0)
        };
        let drives = Drives {
            energy: 0.2,
            sleepiness: 0.4,
            boredom: 0.0,
            curiosity: 0.2,
            ..Drives::default()
        };
        let mut ctx = idle_ctx();
        ctx.user_active = true;
        ctx.idle_s = 0.0;
        let mut moved = false;
        let mut t = 60_000u64;
        while t < 600_000 {
            t += 500;
            brain.update(t, 0.5, &ctx, &body, &world, &drives);
            if matches!(
                brain.state,
                BehaviourState::Walk
                    | BehaviourState::Run
                    | BehaviourState::Jumping
                    | BehaviourState::ChaseCursor
            ) {
                moved = true;
                break;
            }
        }
        assert!(moved, "still stuck sitting after simulated 9 minutes");
    }

    #[test]
    fn heavy_cpu_triggers_react_load() {
        let world = test_world();
        let mut brain = Brain::new(0);
        brain.set_state(BehaviourState::Walk, 60_000);
        let mut body = Body::new(500.0, 1040.0);
        body.grounded_surface = Some(1);
        let drives = Drives::default();
        let mut ctx = idle_ctx();
        ctx.cpu_01 = 0.95;
        // Min duration must elapse first (Walk: 1800 ms).
        let intent = brain.update(62_000, 0.016, &ctx, &body, &world, &drives);
        assert_eq!(brain.state, BehaviourState::ReactLoad);
        assert_eq!(intent.kind, IntentKind::ReactLoad);
        // …and then it cools down instead of retriggering forever
        // (min duration 2500 ms must elapse first).
        brain.update(66_000, 0.016, &ctx, &body, &world, &drives);
        assert_ne!(brain.state, BehaviourState::ReactLoad);
    }

    #[test]
    fn tortoise_unflappable_does_not_startle_and_rabbit_startles() {
        let world = test_world();
        let mut body = Body::new(500.0, 1040.0);
        body.grounded_surface = Some(1);
        let drives = Drives::default();

        // Tortoise has low skittishness (unflappable).
        let mut kaiju_brain = Brain::new(0);
        let mut kaiju_ctx = idle_ctx();
        kaiju_ctx.species = crate::species::SpeciesId::Tortoise;
        kaiju_ctx.personality = crate::species::SpeciesId::Tortoise.default_personality();
        kaiju_ctx.cursor_approaching_fast = true;
        kaiju_ctx.cursor_dist = 180.0;
        kaiju_brain.update(5_000, 0.016, &kaiju_ctx, &body, &world, &drives);
        assert_ne!(
            kaiju_brain.state,
            BehaviourState::Startled,
            "Tortoise should be unflappable and not startle"
        );

        // Rabbit has a very skittish personality.
        let mut shadow_brain = Brain::new(0);
        let mut shadow_ctx = idle_ctx();
        shadow_ctx.species = crate::species::SpeciesId::Rabbit;
        shadow_ctx.personality = crate::species::SpeciesId::Rabbit.default_personality();
        shadow_ctx.cursor_approaching_fast = true;
        shadow_ctx.cursor_dist = 180.0;
        shadow_brain.update(5_000, 0.016, &shadow_ctx, &body, &world, &drives);
        assert_eq!(
            shadow_brain.state,
            BehaviourState::Startled,
            "Rabbit should be skittish and startle"
        );
    }

    #[test]
    fn fox_boldness_triggers_wall_climbing() {
        use crate::world::WallSide;
        let mut world = test_world();
        world.supports.push(Support {
            id: 2,
            kind: SupportKind::Window,
            x1: 200.0,
            x2: 420.0,
            y: 600.0,
            y_bottom: 600.0,
            monitor: 0,
            generation: 1,
            stable: true,
        });
        world.supports.push(Support::new_wall(
            30,
            WallSide::Left,
            420.0,
            400.0,
            900.0,
            0,
            1,
            true,
        ));
        let mut body = Body::new(410.0, 600.0);
        body.grounded_surface = Some(2);

        let mut climber_brain = Brain::new(0);
        let mut climber_ctx = idle_ctx();
        climber_ctx.species = crate::species::SpeciesId::Fox;
        climber_ctx.personality = crate::species::SpeciesId::Fox.default_personality();
        let drives = Drives {
            curiosity: 0.8,
            boredom: 0.2,
            energy: 0.8,
            ..Drives::default()
        };

        climber_brain.set_state(BehaviourState::Idle, 0);
        // Wait past min duration of Idle (1200ms)
        let intent = climber_brain.update(2000, 0.016, &climber_ctx, &body, &world, &drives);
        assert_eq!(
            climber_brain.state,
            BehaviourState::Climbing,
            "Fox near wall should choose climbing"
        );
        assert_eq!(intent.kind, IntentKind::ClimbTo);
    }

    #[test]
    fn cat_curious_on_window_chooses_peeking() {
        let mut world = test_world();
        world.supports.push(Support {
            id: 20,
            kind: SupportKind::Window,
            x1: 200.0,
            x2: 800.0,
            y: 500.0,
            y_bottom: 500.0,
            monitor: 0,
            generation: 1,
            stable: true,
        });
        let mut body = Body::new(300.0, 500.0);
        body.grounded_surface = Some(20);

        let mut peeker_brain = Brain::new(0);
        let mut peeker_ctx = idle_ctx();
        peeker_ctx.species = crate::species::SpeciesId::Cat;
        peeker_ctx.personality = crate::species::SpeciesId::Cat.default_personality();
        let drives = Drives {
            curiosity: 0.85,
            energy: 0.7,
            ..Drives::default()
        };

        peeker_brain.set_state(BehaviourState::Idle, 0);
        let intent = peeker_brain.update(2000, 0.016, &peeker_ctx, &body, &world, &drives);
        assert_eq!(
            peeker_brain.state,
            BehaviourState::Peeking,
            "Cat on window should choose peeking"
        );
        assert_eq!(intent.kind, IntentKind::Peek);
    }

    #[test]
    fn fox_lazy_on_window_curls_into_nap() {
        let mut world = test_world();
        world.supports.push(Support {
            id: 20,
            kind: SupportKind::Window,
            x1: 200.0,
            x2: 800.0,
            y: 500.0,
            y_bottom: 500.0,
            monitor: 0,
            generation: 1,
            stable: true,
        });
        let mut body = Body::new(300.0, 500.0);
        body.grounded_surface = Some(20);

        let mut ringtail_brain = Brain::new(0);
        let mut ringtail_ctx = idle_ctx();
        ringtail_ctx.species = crate::species::SpeciesId::Fox;
        ringtail_ctx.personality = crate::species::SpeciesId::Fox.default_personality();
        let drives = Drives {
            sleepiness: 0.5,
            energy: 0.4,
            curiosity: 0.2,
            boredom: 0.1,
            ..Drives::default()
        };

        ringtail_brain.set_state(BehaviourState::Idle, 0);
        let intent = ringtail_brain.update(25_000, 0.016, &ringtail_ctx, &body, &world, &drives);
        assert_eq!(
            ringtail_brain.state,
            BehaviourState::Sleep,
            "Fox on window should curl up to sleep"
        );
        assert_eq!(intent.kind, IntentKind::Sleep);
    }

    #[test]
    fn climber_scrambles_onto_top_window_and_stops_climbing() {
        use crate::world::WallSide;
        let mut world = test_world();
        let win = Support {
            id: 2,
            kind: SupportKind::Window,
            x1: 400.0,
            x2: 800.0,
            y: 300.0,
            y_bottom: 300.0,
            monitor: 0,
            generation: 1,
            stable: true,
        };
        let wall = Support::new_wall(5, WallSide::Left, 400.0, 300.0, 800.0, 0, 1, true);
        world.supports.push(win);
        world.supports.push(wall);

        let cfg = SimConfig::default();
        let sense = crate::SenseInput::default();
        let mut sim = crate::CreatureSim::new_with_species(
            1,
            crate::species::SpeciesId::Fox,
            400.0,
            307.0, // Just below the top (s.y + 8.0 = 308.0)
            0,
        );
        sim.body.grounded_surface = Some(5);
        sim.brain.set_state(BehaviourState::Climbing, 0);

        // Tick 1: Reaches top, scrambles to top window support (id: 2)
        sim.tick(0.016, 16, &sense, &world, &cfg);
        assert_eq!(sim.body.grounded_surface, Some(2));
        assert_eq!(sim.body.pos.x, 418.0);
        assert_eq!(sim.body.pos.y, 300.0);

        // Tick 2: Brain updates. It must transition out of Climbing and NOT flap back to the wall!
        sim.tick(0.016, 32, &sense, &world, &cfg);
        assert_ne!(
            sim.brain.state,
            BehaviourState::Climbing,
            "Must exit Climbing state once scrambled onto window ledge"
        );
        assert_eq!(
            sim.body.grounded_surface,
            Some(2),
            "Must remain securely grounded on the window ledge without flapping back to wall"
        );
        assert_eq!(sim.body.pos.x, 418.0);
    }

    #[test]
    fn config_unused_ok() {
        let _ = SimConfig::default();
        let _ = Personality::default();
    }

    #[test]
    fn tortoise_rejects_forbidden_voluntary_behaviours() {
        use crate::world::WallSide;
        let mut world = test_world();
        world.supports.push(Support {
            id: 2,
            kind: SupportKind::Window,
            x1: 200.0,
            x2: 700.0,
            y: 700.0,
            y_bottom: 700.0,
            monitor: 0,
            generation: 1,
            stable: true,
        });
        world.supports.push(Support::new_wall(
            3,
            WallSide::Left,
            200.0,
            500.0,
            900.0,
            0,
            1,
            true,
        ));
        let body = Body {
            grounded_surface: Some(2),
            ..Body::new(400.0, 700.0)
        };
        let mut ctx = idle_ctx();
        ctx.species = crate::species::SpeciesId::Tortoise;
        ctx.personality = ctx.species.default_personality();
        let drives = Drives {
            energy: 1.0,
            boredom: 1.0,
            curiosity: 1.0,
            ..Drives::default()
        };
        let mut brain = Brain::new(0);
        let intent = brain.update(2_000, 0.016, &ctx, &body, &world, &drives);
        assert!(!matches!(
            brain.state,
            BehaviourState::Run
                | BehaviourState::ChaseCursor
                | BehaviourState::Climbing
                | BehaviourState::Jumping
        ));
        assert!(!matches!(
            intent.kind,
            IntentKind::RunTo | IntentKind::JumpTo | IntentKind::ClimbTo
        ));
    }

    #[test]
    fn tortoise_come_here_is_a_walk_command() {
        let world = test_world();
        let body = Body {
            grounded_surface: Some(1),
            ..Body::new(500.0, 1040.0)
        };
        let mut ctx = idle_ctx();
        ctx.species = crate::species::SpeciesId::Tortoise;
        ctx.personality = ctx.species.default_personality();
        let mut brain = Brain::new(0);
        brain.command = Some(AppCommand::ComeHere { x: 600.0, y: 0.0 });
        let intent = brain.update(100, 0.016, &ctx, &body, &world, &Drives::default());
        assert_eq!(brain.state, BehaviourState::Walk);
        assert_eq!(intent.kind, IntentKind::WalkTo);
    }
}
