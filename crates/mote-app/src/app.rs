//! App: the main loop that makes Mote live.
//!
//! One UI thread, two timers, zero polling excess:
//! - 16 ms tick: cursor, idle, fixed-step sim, animator, overlay blit;
//! - 1 s tick: CPU / memory / fullscreen;
//! - audio meter at ~10 Hz; window list rebuilt on WinEvent notifications
//!   (2 s fallback so a missed event can never desync Mote).

use std::collections::VecDeque;
use std::time::Instant;

use mote_core::{BehaviourState, CreatureSim, SenseInput, SimConfig, SpeciesId, WorldSnapshot};
use mote_render::Animator;
use mote_win::{
    build_world, enumerate_windows, foreground_is_fullscreen, idle_ms, poll_cursor, query_memory,
    query_monitors, query_taskbar, AudioMonitor, CpuMeter, CursorTracker, WinEventListener,
};
use windows::core::w;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, ReleaseCapture, SetCapture, UnregisterHotKey, MOD_ALT, MOD_CONTROL,
    MOD_NOREPEAT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, DestroyWindow, GetWindowLongPtrW, KillTimer, PostQuitMessage,
    RegisterWindowMessageW, SetTimer, SetWindowLongPtrW, GWLP_USERDATA, HTCLIENT, HTTRANSPARENT,
    PBT_APMRESUMEAUTOMATIC, PBT_APMSUSPEND, WM_CLOSE, WM_CREATE, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_HOTKEY, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE,
    WM_NCHITTEST, WM_POWERBROADCAST, WM_RBUTTONUP, WM_TIMER,
};

use crate::overlay::{Overlay, OVERLAY_PX};
use crate::settings::{save as save_settings, Settings};
use crate::tray::{MenuAction, TrayIcon, TRAY_CALLBACK_MSG};
use mote_core::behaviour::AppCommand;

const TICK_TIMER: usize = 1;
const SLOW_TIMER: usize = 2;
const TICK_MS: u32 = 32;
const HIDE_HOTKEY: i32 = 1;

pub struct MoteInstance {
    pub id: u32,
    pub species: SpeciesId,
    pub sim: CreatureSim,
    pub animator: Animator,
    pub overlay: Option<Overlay>,
    pub drag: Option<DragState>,
    pub pet_blush: f32,
    pub land_impulse: f32,
    pub last_state: BehaviourState,
    /// Pixels and origin of the last presented frame, used for precise hit testing.
    pub presented_frame: Vec<u8>,
    pub presented_origin: (i32, i32),
}

pub struct DragState {
    pub off_x: f32,
    pub off_y: f32,
    pub start_x: f32,
    pub start_y: f32,
    pub moved: bool,
    pub trail: VecDeque<(f64, f32, f32)>,
}

pub struct App {
    hwnd: HWND,
    tray: Option<TrayIcon>,
    pub settings: Settings,
    pub instances: Vec<MoteInstance>,
    cfg: SimConfig,
    world: WorldSnapshot,
    world_generation: u64,
    cursor: CursorTracker,
    cpu: CpuMeter,
    audio: AudioMonitor,
    win_events: WinEventListener,
    taskbar_created_msg: u32,
    start: Instant,
    last_tick: Instant,
    tick_count: u64,
    last_world_rebuild_ms: u64,
    last_slow_ms: u64,
    last_audio_poll_s: f64,
    fullscreen_paused: bool,
    hidden_manual: bool,
    hide_until_ms: u64,
    suspended: bool,
    picker_open: bool,
    cadence_ms: u32,
    last_cursor: (i32, i32),
}

impl App {
    pub fn new(mut settings: Settings) -> Self {
        settings.set_lineup(settings.lineup());
        let start = Instant::now();
        let radius = settings.size.radius();
        let monitors = query_monitors();
        let taskbar = query_taskbar(&monitors);
        let windows = enumerate_windows();
        let mut world = build_world(&mote_win::world_build::WorldInputs {
            monitors: &monitors,
            taskbar: &taskbar,
            windows: &windows,
            generation: 1,
        });
        if !settings.allow_climbing {
            strip_window_supports(&mut world);
        }
        // Spawn: centre of the preferred monitor's taskbar, else screen floor.
        let (sx, sy) = spawn_point(&monitors, &taskbar, settings.preferred_monitor);
        let count = settings.mote_count.clamp(1, 4) as usize;
        let mut instances = Vec::with_capacity(count);
        let vr = &world.virtual_rect;

        for i in 0..count {
            let species = settings.pets[i];
            let offset_x = (i as f32 - (count as f32 - 1.0) / 2.0) * 110.0;
            let spawn_x = (sx + offset_x).clamp(vr.x as f32 + 50.0, (vr.x + vr.w) as f32 - 50.0);
            let mut sim = CreatureSim::new_with_species(
                (i + 1) as u32,
                species,
                spawn_x,
                sy,
                (i as u64) * 400,
            );
            if let Some(t) = world.taskbar() {
                if spawn_x >= t.x1 && spawn_x <= t.x2 {
                    sim.body.pos.y = t.y;
                    sim.body.grounded_surface = Some(t.id);
                }
            }
            instances.push(MoteInstance {
                id: (i + 1) as u32,
                species,
                sim,
                animator: Animator::new(radius),
                overlay: None,
                drag: None,
                pet_blush: 0.0,
                land_impulse: 0.0,
                last_state: BehaviourState::Idle,
                presented_frame: Vec::new(),
                presented_origin: (0, 0),
            });
        }

        Self {
            hwnd: HWND::default(),
            tray: None,
            settings,
            instances,
            cfg: SimConfig::default(),
            world,
            world_generation: 1,
            cursor: CursorTracker::new(),
            cpu: CpuMeter::new(),
            audio: AudioMonitor::new(),
            win_events: WinEventListener::install(),
            taskbar_created_msg: 0,
            start,
            last_tick: start,
            tick_count: 0,
            last_world_rebuild_ms: 0,
            last_slow_ms: 0,
            last_audio_poll_s: f64::NEG_INFINITY,
            fullscreen_paused: false,
            hidden_manual: false,
            hide_until_ms: 0,
            suspended: false,
            picker_open: false,
            cadence_ms: TICK_MS,
            last_cursor: (sx as i32, sy as i32 - 200),
        }
    }

    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    fn now_s(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn set_cadence(&mut self, millis: u32) {
        if self.cadence_ms != millis && !self.hwnd.0.is_null() {
            self.cadence_ms = millis;
            unsafe {
                let _ = SetTimer(Some(self.hwnd), TICK_TIMER, millis, None);
            }
        }
    }

    fn create_instance_overlay(&self) -> Option<Overlay> {
        unsafe {
            let instance: HINSTANCE = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .ok()?
                .into();
            let app_ptr = self as *const App as *mut App as *mut std::ffi::c_void;
            Overlay::create(instance, app_ptr).ok()
        }
    }

    pub fn sync_mote_count(&mut self) {
        let target_count = self.settings.mote_count.clamp(1, 4) as usize;
        if self.instances.len() > target_count {
            for inst in &self.instances[target_count..] {
                if inst.drag.is_some() {
                    unsafe {
                        let _ = ReleaseCapture();
                    }
                }
            }
            self.instances.truncate(target_count);
        } else if self.instances.len() < target_count {
            let monitors = query_monitors();
            let taskbar = query_taskbar(&monitors);
            let (sx, sy) = spawn_point(&monitors, &taskbar, self.settings.preferred_monitor);
            let radius = self.settings.size.radius();
            while self.instances.len() < target_count {
                let i = self.instances.len();
                let species = self.settings.pets[i];
                let dir = if i % 2 == 1 { 1.0 } else { -1.0 };
                let offset_x = (i as f32 * 90.0) * dir;
                let vr = &self.world.virtual_rect;
                let spawn_x =
                    (sx + offset_x).clamp(vr.x as f32 + 50.0, (vr.x + vr.w) as f32 - 50.0);
                let mut sim = CreatureSim::new_with_species(
                    (i + 1) as u32,
                    species,
                    spawn_x,
                    sy,
                    (i as u64) * 400,
                );
                if let Some(t) = self.world.taskbar() {
                    if spawn_x >= t.x1 && spawn_x <= t.x2 {
                        sim.body.pos.y = t.y;
                        sim.body.grounded_surface = Some(t.id);
                    }
                }
                let overlay = if self.hwnd.0.is_null() {
                    None
                } else {
                    self.create_instance_overlay()
                };
                self.instances.push(MoteInstance {
                    id: (i + 1) as u32,
                    species,
                    sim,
                    animator: Animator::new(radius),
                    overlay,
                    drag: None,
                    pet_blush: 0.0,
                    land_impulse: 0.0,
                    last_state: BehaviourState::Idle,
                    presented_frame: Vec::new(),
                    presented_origin: (0, 0),
                });
            }
        }
        // Ensure all active instances have their overlay window created if attached
        if !self.hwnd.0.is_null() {
            for i in 0..self.instances.len() {
                if self.instances[i].overlay.is_none() {
                    let overlay = self.create_instance_overlay();
                    self.instances[i].overlay = overlay;
                }
            }
        }
        for inst in &self.instances {
            if let Some(o) = &inst.overlay {
                o.set_input_passthrough(self.settings.click_through);
            }
        }
    }

    pub fn set_lineup(&mut self, pets: Vec<SpeciesId>) {
        self.settings.set_lineup(pets);
        self.sync_mote_count();
        for (inst, &species) in self.instances.iter_mut().zip(&self.settings.pets) {
            if inst.species != species {
                inst.species = species;
                inst.sim.set_species(species);
                inst.animator = Animator::new(self.settings.size.radius());
                inst.presented_frame.clear();
            }
        }
        if let Some(t) = self.tray.as_mut() {
            t.update_species(self.settings.species);
        }
    }

    pub fn open_picker(&mut self) {
        match crate::picker::show_picker(
            self.hwnd,
            self.settings.lineup(),
            self.settings.quiet(),
            self.settings.reduce_motion,
        ) {
            Ok(()) => self.picker_open = true,
            Err(e) => log::error!("could not open pet picker: {e}"),
        }
    }

    /// Called once the initial overlay window exists.
    pub fn attach(&mut self, hwnd: HWND, overlay: Overlay) {
        self.hwnd = hwnd;
        if !self.instances.is_empty() {
            self.instances[0].overlay = Some(overlay);
        }
        self.tray = Some(TrayIcon::new(hwnd, self.settings.species));
        self.taskbar_created_msg = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        unsafe {
            let _ = SetTimer(Some(hwnd), TICK_TIMER, TICK_MS, None);
            let _ = SetTimer(Some(hwnd), SLOW_TIMER, 1000, None);
            if RegisterHotKey(
                Some(hwnd),
                HIDE_HOTKEY,
                MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
                b'M' as u32,
            )
            .is_err()
            {
                log::warn!("Ctrl+Alt+M already in use; hide remains available from the tray");
            }
        }
        self.sync_mote_count();
        self.cpu.sample();
        log::info!(
            "Mote attached and running with {} instance(s)",
            self.instances.len()
        );
    }

    fn rebuild_world(&mut self) {
        let monitors = query_monitors();
        let taskbar = query_taskbar(&monitors);
        let windows = enumerate_windows();
        self.world_generation += 1;
        let mut world = build_world(&mote_win::world_build::WorldInputs {
            monitors: &monitors,
            taskbar: &taskbar,
            windows: &windows,
            generation: self.world_generation,
        });
        if !self.settings.allow_climbing {
            strip_window_supports(&mut world);
        }
        self.world = world;
        self.last_world_rebuild_ms = self.now_ms();
        // Clamp all Motes into the new virtual rect (monitor unplugged under it).
        let vr = &self.world.virtual_rect;
        for inst in &mut self.instances {
            inst.sim.body.pos.x = inst.sim.body.pos.x.clamp(vr.x as f32, (vr.x + vr.w) as f32);
            inst.sim.body.pos.y = inst
                .sim
                .body
                .pos
                .y
                .clamp(vr.y as f32, (vr.y + vr.h) as f32 - 2.0);
        }
    }

    fn tick(&mut self) {
        if self.suspended {
            return;
        }
        let now = Instant::now();
        let mut dt = (now - self.last_tick).as_secs_f32();
        self.last_tick = now;
        dt = dt.clamp(0.001, 0.05);
        let now_ms = self.now_ms();
        let now_s = self.now_s();
        self.tick_count += 1;

        // --- Cursor + idle (cheap, every tick).
        if let Some((cx, cy)) = poll_cursor() {
            self.last_cursor = (cx, cy);
        }
        let sample = self
            .cursor
            .push(self.last_cursor.0 as f32, self.last_cursor.1 as f32, now_s);
        let idle = idle_ms();
        let idle_capped = if idle == u64::MAX { 0 } else { idle };
        let user_active = idle_capped < 5_000;

        // --- Audio at ~10 Hz.
        if self.settings.music_reactions && now_s - self.last_audio_poll_s > 0.1 {
            self.last_audio_poll_s = now_s;
            self.audio.poll(now_s);
        }
        // --- Slow sensors at ~1 Hz.
        if now_ms - self.last_slow_ms > 1000 {
            self.last_slow_ms = now_ms;
            self.cpu.sample();
            let _mem = query_memory();
            let fs = foreground_is_fullscreen();
            let want_pause = self.settings.pause_on_fullscreen && fs;
            if want_pause != self.fullscreen_paused {
                self.fullscreen_paused = want_pause;
                if want_pause {
                    log::info!("fullscreen app active: pausing overlay");
                    for inst in &self.instances {
                        if let Some(o) = &inst.overlay {
                            o.set_visible(false);
                        }
                    }
                } else {
                    log::info!("fullscreen app gone: resuming");
                    self.rebuild_world();
                }
            }
        }
        if self.fullscreen_paused || self.hidden_manual || self.picker_open {
            for inst in &self.instances {
                if let Some(o) = &inst.overlay {
                    o.set_visible(false);
                }
            }
            self.set_cadence(250);
            return;
        }

        // --- World refresh: event-driven + 2 s fallback.
        if self.win_events.poll_changed() || now_ms - self.last_world_rebuild_ms > 2000 {
            self.rebuild_world();
        }

        let positions: Vec<(f32, f32)> = self
            .instances
            .iter()
            .map(|i| (i.sim.body.pos.x, i.sim.body.pos.y))
            .collect();

        let hidden = self.hidden_manual || now_ms < self.hide_until_ms || self.fullscreen_paused;
        let primary_sleeping = self
            .instances
            .first()
            .map(|i| i.sim.brain.state == BehaviourState::Sleep)
            .unwrap_or(false);

        for (idx, inst) in self.instances.iter_mut().enumerate() {
            // --- Assemble senses (settings gates applied here).
            let (sense_cursor_x, sense_cursor_y, sense_speed) = if self.settings.cursor_interactions
            {
                (sample.x, sample.y, sample.speed_px_s)
            } else {
                (inst.sim.body.pos.x + 5000.0, inst.sim.body.pos.y, 0.0)
            };
            let cpu = if self.settings.cpu_reactions {
                self.cpu.usage
            } else {
                0.0
            };
            let media_playing = self.settings.music_reactions && self.audio.playing;
            let sense = SenseInput {
                cursor_x: sense_cursor_x,
                cursor_y: sense_cursor_y,
                cursor_vx: sample.vx,
                cursor_vy: sample.vy,
                cursor_speed_px_s: sense_speed,
                idle_ms: idle_capped,
                user_active,
                cpu_01: cpu,
                mem_01: 0.0,
                audio_level_01: if self.settings.music_reactions {
                    self.audio.level
                } else {
                    0.0
                },
                media_playing,
                fullscreen_app_active: false,
                monitor_count: self.world.monitors.len() as u32,
            };

            // Inter-Mote proximity & companionship
            let mut nearest_sibling = None;
            let mut min_dist = f32::MAX;
            for (other_idx, &(ox, oy)) in positions.iter().enumerate() {
                if other_idx == idx {
                    continue;
                }
                let dx = ox - inst.sim.body.pos.x;
                let dy = oy - inst.sim.body.pos.y;
                let d = (dx * dx + dy * dy).sqrt();
                if d < min_dist {
                    min_dist = d;
                    nearest_sibling = Some((ox, oy, d));
                }
            }

            if let Some((nx, _ny, dist)) = nearest_sibling {
                if dist < 80.0
                    && inst.sim.body.grounded()
                    && inst.sim.brain.state == BehaviourState::Idle
                {
                    // Face companion and show gentle interest
                    inst.sim.body.facing = if nx > inst.sim.body.pos.x { 1 } else { -1 };
                    inst.sim.drives.curiosity = (inst.sim.drives.curiosity + dt * 0.25).min(1.0);
                }
            }

            // --- Drag overrides the sim position.
            if inst.drag.is_some() {
                inst.sim.brain.forced_state = Some(BehaviourState::Dragged);
            }

            // --- Step the simulation.
            let events = inst.sim.tick(dt, now_ms, &sense, &self.world, &self.cfg);
            for e in &events {
                if let mote_core::physics::PhysicsEvent::Landed { impact_px_s, .. } = e {
                    inst.land_impulse = (*impact_px_s / 1100.0)
                        .clamp(0.15, 1.0)
                        .max(inst.land_impulse);
                }
            }

            // --- Animate.
            let gaze = if let Some((nx, ny, dist)) = nearest_sibling {
                if dist < 220.0 && (!user_active || !self.settings.cursor_interactions) {
                    compute_gaze(nx, ny, inst.sim.body.pos.x, inst.sim.body.pos.y)
                } else {
                    compute_gaze(
                        self.last_cursor.0 as f32,
                        self.last_cursor.1 as f32,
                        inst.sim.body.pos.x,
                        inst.sim.body.pos.y,
                    )
                }
            } else {
                compute_gaze(
                    self.last_cursor.0 as f32,
                    self.last_cursor.1 as f32,
                    inst.sim.body.pos.x,
                    inst.sim.body.pos.y,
                )
            };

            let blush = std::mem::replace(&mut inst.pet_blush, 0.0);
            let land = std::mem::replace(&mut inst.land_impulse, 0.0);
            let anim_in = mote_render::anim::AnimInput {
                state: inst.sim.brain.state,
                horizontal_speed: inst.sim.body.vel.x.abs(),
                vel_y: inst.sim.body.vel.y,
                facing: inst.sim.body.facing,
                gaze,
                audio_level: if self.settings.music_reactions {
                    self.audio.level
                } else {
                    0.0
                },
                blush_push: blush,
                land_impulse: land,
                species: inst.species,
                reduce_motion: self.settings.reduce_motion,
            };
            inst.animator.update(dt, &anim_in);
            let state_changed = inst.sim.brain.state != inst.last_state;
            if state_changed {
                log::info!(
                    "mote {} ({:?}) state {:?} -> {:?} at ({:.0},{:.0})",
                    inst.id,
                    inst.species,
                    inst.last_state,
                    inst.sim.brain.state,
                    inst.sim.body.pos.x,
                    inst.sim.body.pos.y
                );
            }
            inst.last_state = inst.sim.brain.state;

            // --- Render cadence: 30 fps awake, ~7 fps asleep, none when hidden.
            let sleeping = inst.sim.brain.state == BehaviourState::Sleep;
            let want_frame =
                !hidden && (state_changed || !sleeping || self.tick_count.is_multiple_of(2));
            if want_frame {
                let pose = inst.animator.pose(&anim_in);
                let frame = mote_render::creature::draw_mote(&pose);
                if let Some(o) = inst.overlay.as_mut() {
                    o.set_pixels(&frame);
                    let s = self.settings.size.radius() / 64.0;
                    let wx = (inst.sim.body.pos.x - OVERLAY_PX as f32 / 2.0) as i32;
                    let wy = (inst.sim.body.pos.y
                        - OVERLAY_PX as f32 / 2.0
                        - mote_render::FEET_BELOW_CENTER * s) as i32;
                    o.present(wx, wy);
                    if !o.present_failed() {
                        inst.presented_origin = (wx, wy);
                        inst.presented_frame = frame;
                    }
                    o.set_visible(true);
                    if o.present_failed() && self.tick_count.is_multiple_of(600) {
                        log::warn!("overlay present failing for mote {}", inst.id);
                    }
                }
            } else if hidden {
                if let Some(o) = &inst.overlay {
                    o.set_visible(false);
                }
            }
        }

        self.set_cadence(
            if self
                .instances
                .iter()
                .all(|i| i.sim.brain.state == BehaviourState::Sleep)
            {
                64
            } else {
                TICK_MS
            },
        );

        // --- Tray tip refresh every ~5 s.
        if self.tick_count.is_multiple_of(300) {
            if let Some(t) = &self.tray {
                t.update_tip(primary_sleeping, hidden);
            }
        }

        // --- Heartbeat: proves the tick loop is alive. ~1 line/min at 16 ms ticks.
        if self.tick_count.is_multiple_of(3600) {
            log::info!(
                "heartbeat: {} mote(s) active, idle={}ms cpu={:.0}% audio={:.2}",
                self.instances.len(),
                idle_capped,
                self.cpu.usage * 100.0,
                self.audio.level,
            );
        }
    }

    fn handle_message(
        &mut self,
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        match msg {
            WM_CLOSE => {
                self.apply_menu_action(MenuAction::Quit);
                Some(LRESULT(0))
            }
            crate::picker::WM_APP_OPEN_PICKER => {
                self.open_picker();
                Some(LRESULT(0))
            }
            crate::picker::WM_APP_PICKER_RESULT => {
                if let Some(result) = crate::picker::take_result(self.hwnd) {
                    self.picker_open = false;
                    if !result.cancelled {
                        self.set_lineup(result.pets);
                        if self.settings.quiet() != result.quiet {
                            self.settings.set_quiet(result.quiet);
                        }
                        self.settings.reduce_motion = result.reduced_motion;
                        self.hidden_manual = false;
                        self.hide_until_ms = 0;
                        self.rebuild_world();
                        save_settings(&self.settings);
                    }
                    self.last_tick = Instant::now();
                }
                Some(LRESULT(0))
            }
            WM_HOTKEY if wparam.0 == HIDE_HOTKEY as usize => {
                self.apply_menu_action(MenuAction::HideShow);
                Some(LRESULT(0))
            }
            WM_TIMER => {
                match wparam.0 {
                    TICK_TIMER => self.tick(),
                    SLOW_TIMER => {}
                    _ => {}
                }
                Some(LRESULT(0))
            }
            WM_NCHITTEST => Some(self.hit_test(hwnd, lparam)),
            WM_LBUTTONDOWN => {
                self.on_lbutton_down(hwnd);
                Some(LRESULT(0))
            }
            WM_MOUSEMOVE => {
                self.on_mouse_move();
                Some(LRESULT(0))
            }
            WM_LBUTTONUP => {
                self.on_lbutton_up();
                Some(LRESULT(0))
            }
            WM_RBUTTONUP => {
                self.on_context_menu();
                Some(LRESULT(0))
            }
            WM_DISPLAYCHANGE | WM_DPICHANGED => {
                log::info!("display changed: rebuilding world");
                self.rebuild_world();
                Some(LRESULT(0))
            }
            WM_POWERBROADCAST => {
                match wparam.0 as u32 {
                    PBT_APMSUSPEND => {
                        log::info!("suspend: saving settings");
                        save_settings(&self.settings);
                        self.suspended = true;
                    }
                    PBT_APMRESUMEAUTOMATIC => {
                        log::info!("resume: rebuilding world");
                        self.suspended = false;
                        self.last_tick = Instant::now();
                        self.rebuild_world();
                    }
                    _ => {}
                }
                Some(LRESULT(1))
            }
            _ => {
                if msg == TRAY_CALLBACK_MSG {
                    self.on_tray_event(lparam);
                    return Some(LRESULT(0));
                }
                if msg == self.taskbar_created_msg && msg != 0 {
                    log::info!("taskbar recreated (Explorer restart)");
                    if let Some(t) = &self.tray {
                        t.add();
                    }
                    self.rebuild_world();
                    return Some(LRESULT(0));
                }
                None
            }
        }
    }

    // -- Hit testing: only the creature's body is "solid". -----------------
    fn hit_test(&self, hwnd: HWND, lparam: LPARAM) -> LRESULT {
        if self.settings.click_through
            || self.picker_open
            || self.hidden_manual
            || self.fullscreen_paused
        {
            return LRESULT(HTTRANSPARENT as isize);
        }
        let x = (lparam.0 & 0xFFFF) as i16 as i32;
        let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
        if let Some(inst) = self
            .instances
            .iter()
            .find(|i| i.overlay.as_ref().map(|o| o.hwnd) == Some(hwnd))
        {
            // When peeking behind a window edge, lower body is clipped transparent;
            // any click at or below the ledge line must pass through to the underlying window.
            if inst.sim.brain.state == BehaviourState::Peeking && (y as f32) >= inst.sim.body.pos.y
            {
                return LRESULT(HTTRANSPARENT as isize);
            }

            let px = x - inst.presented_origin.0;
            let py = y - inst.presented_origin.1;
            let on_art = (0..OVERLAY_PX).contains(&px)
                && (0..OVERLAY_PX).contains(&py)
                && inst
                    .presented_frame
                    .get(((py * OVERLAY_PX + px) * 4 + 3) as usize)
                    .is_some_and(|alpha| *alpha > 64);
            if inst.drag.is_some() || on_art {
                return LRESULT(HTCLIENT as isize);
            }
        }
        LRESULT(HTTRANSPARENT as isize)
    }

    fn on_lbutton_down(&mut self, hwnd: HWND) {
        if let Some((fx, fy)) = poll_cursor() {
            self.last_cursor = (fx, fy);
        }
        let (cx, cy) = self.last_cursor;
        if let Some(inst) = self
            .instances
            .iter_mut()
            .find(|i| i.overlay.as_ref().map(|o| o.hwnd) == Some(hwnd))
        {
            log::info!(
                "mousedown at {cx},{cy} (mote {} at {:.0},{:.0})",
                inst.id,
                inst.sim.body.pos.x,
                inst.sim.body.pos.y
            );
            inst.drag = Some(DragState {
                off_x: inst.sim.body.pos.x - cx as f32,
                off_y: inst.sim.body.pos.y - cy as f32,
                start_x: cx as f32,
                start_y: cy as f32,
                moved: false,
                trail: VecDeque::with_capacity(16),
            });
            unsafe {
                let _ = SetCapture(hwnd);
            }
        }
    }

    fn on_mouse_move(&mut self) {
        if let Some((fx, fy)) = poll_cursor() {
            self.last_cursor = (fx, fy);
        }
        let (cx, cy) = (self.last_cursor.0 as f32, self.last_cursor.1 as f32);
        let t = self.now_s();
        let vr = self.world.virtual_rect;

        for inst in self.instances.iter_mut().filter(|i| i.drag.is_some()) {
            let (off_x, off_y) = {
                let d = inst.drag.as_ref().unwrap();
                (d.off_x, d.off_y)
            };
            {
                let d = inst.drag.as_mut().unwrap();
                if ((cx - d.start_x).abs() > 6.0) || ((cy - d.start_y).abs() > 6.0) {
                    d.moved = true;
                }
            }
            inst.sim.body.pos.x = (cx + off_x).clamp(vr.x as f32, (vr.x + vr.w) as f32);
            inst.sim.body.pos.y = (cy + off_y).clamp(vr.y as f32, (vr.y + vr.h) as f32);
            inst.sim.body.vel.x = 0.0;
            inst.sim.body.vel.y = 0.0;
            inst.sim.body.grounded_surface = None;
            {
                let d = inst.drag.as_mut().unwrap();
                d.trail
                    .push_back((t, inst.sim.body.pos.x, inst.sim.body.pos.y));
                while d.trail.len() > 12 {
                    d.trail.pop_front();
                }
            }
        }
    }

    fn on_lbutton_up(&mut self) {
        unsafe {
            let _ = ReleaseCapture();
        }
        let now_s = self.now_s();
        for inst in &mut self.instances {
            if let Some(d) = inst.drag.take() {
                log::info!(
                    "mouseup mote {} moved={} trail={}",
                    inst.id,
                    d.moved,
                    d.trail.len()
                );
                if !d.moved {
                    // Click / pet: happy hop + blush + a little excitement.
                    inst.pet_blush = 0.9;
                    inst.sim.drives.spike(0.35, 0.0);
                    if inst.sim.body.grounded() {
                        inst.sim.body.vel.y = -320.0;
                        inst.sim.body.grounded_surface = None;
                    }
                    log::debug!("mote {} petted", inst.id);
                } else {
                    // Throw: velocity from the recent drag trail.
                    let (vx, vy) = throw_velocity(&d.trail, now_s);
                    inst.sim.body.throw_with(vx, vy);
                    inst.sim.brain.forced_state = Some(BehaviourState::Falling);
                    log::debug!("mote {} thrown vx={vx:.0} vy={vy:.0}", inst.id);
                }
            }
        }
    }

    fn on_context_menu(&mut self) {
        log::info!("contextmenu requested");
        let primary_sleeping = self
            .instances
            .first()
            .map(|i| i.sim.brain.state == BehaviourState::Sleep)
            .unwrap_or(false);
        let hidden = self.hidden_manual;
        let action = match &self.tray {
            Some(t) => t.show_menu(&self.settings, primary_sleeping, hidden),
            None => {
                log::warn!("contextmenu: no tray icon");
                None
            }
        };
        log::info!("contextmenu result: {action:?}");
        if let Some(a) = action {
            self.apply_menu_action(a);
        }
    }

    fn on_tray_event(&mut self, lparam: LPARAM) {
        match lparam.0 as u32 {
            WM_RBUTTONUP => self.on_context_menu(),
            WM_LBUTTONDBLCLK => self.open_picker(),
            _ => {}
        }
    }

    fn apply_menu_action(&mut self, a: MenuAction) {
        match a {
            MenuAction::ChoosePets => self.open_picker(),
            MenuAction::ToggleClickThrough => {
                self.settings.click_through = !self.settings.click_through;
                for inst in &self.instances {
                    if let Some(o) = &inst.overlay {
                        o.set_input_passthrough(self.settings.click_through);
                    }
                }
            }
            MenuAction::SleepWake => {
                let any_awake = self
                    .instances
                    .iter()
                    .any(|i| i.sim.brain.state != BehaviourState::Sleep);
                for inst in &mut self.instances {
                    if any_awake {
                        inst.sim.brain.command = Some(AppCommand::SleepNow);
                    } else {
                        inst.sim.brain.command = Some(AppCommand::WakeNow);
                    }
                }
            }
            MenuAction::HideShow => {
                self.hidden_manual = !self.hidden_manual;
                if !self.hidden_manual {
                    self.hide_until_ms = 0;
                }
            }
            MenuAction::CallMote => {
                self.hidden_manual = false;
                self.hide_until_ms = 0;
                let (cx, _) = self.last_cursor;
                let count = self.instances.len() as f32;
                for (idx, inst) in self.instances.iter_mut().enumerate() {
                    let offset = (idx as f32 - (count - 1.0) / 2.0) * 80.0;
                    inst.sim.brain.command = Some(AppCommand::ComeHere {
                        x: cx as f32 + offset,
                        y: 0.0,
                    });
                }
            }
            MenuAction::SizeSmall => self.set_size(crate::settings::CreatureSize::Small),
            MenuAction::SizeMedium => self.set_size(crate::settings::CreatureSize::Medium),
            MenuAction::SizeLarge => self.set_size(crate::settings::CreatureSize::Large),
            MenuAction::ToggleMusic => {
                self.settings.music_reactions = !self.settings.music_reactions;
            }
            MenuAction::ToggleCursor => {
                self.settings.cursor_interactions = !self.settings.cursor_interactions;
            }
            MenuAction::ToggleCpu => {
                self.settings.cpu_reactions = !self.settings.cpu_reactions;
            }
            MenuAction::ToggleClimb => {
                self.settings.allow_climbing = !self.settings.allow_climbing;
                self.rebuild_world();
            }
            MenuAction::ToggleMotion => {
                self.settings.reduce_motion = !self.settings.reduce_motion;
            }
            MenuAction::ToggleFullscreen => {
                self.settings.pause_on_fullscreen = !self.settings.pause_on_fullscreen;
            }
            MenuAction::ToggleStartup => {
                self.settings.launch_at_startup = !self.settings.launch_at_startup;
                crate::settings::set_startup(self.settings.launch_at_startup);
            }
            MenuAction::Quit => {
                save_settings(&self.settings);
                crate::picker::close_picker(self.hwnd);
                unsafe {
                    let _ = UnregisterHotKey(Some(self.hwnd), HIDE_HOTKEY);
                    let _ = KillTimer(Some(self.hwnd), TICK_TIMER);
                    let _ = KillTimer(Some(self.hwnd), SLOW_TIMER);
                    PostQuitMessage(0);
                    for inst in &self.instances {
                        if let Some(o) = &inst.overlay {
                            if o.hwnd != self.hwnd {
                                let _ = DestroyWindow(o.hwnd);
                            }
                        }
                    }
                    let _ = DestroyWindow(self.hwnd);
                }
            }
        }
        save_settings(&self.settings);
    }

    fn set_size(&mut self, size: crate::settings::CreatureSize) {
        self.settings.size = size;
        for inst in &mut self.instances {
            inst.animator.set_radius(size.radius());
        }
    }
}

fn throw_velocity(trail: &VecDeque<(f64, f32, f32)>, now_s: f64) -> (f32, f32) {
    if trail.len() < 2 {
        return (0.0, -100.0);
    }
    // Use samples within the last 120 ms.
    let cutoff = now_s - 0.12;
    let mut first = trail[0];
    for s in trail.iter() {
        if s.0 >= cutoff {
            first = *s;
            break;
        }
    }
    let last = trail[trail.len() - 1];
    let dt = (last.0 - first.0).max(0.016);
    let vx = ((last.1 - first.1) / dt as f32).clamp(-1100.0, 1100.0);
    let mut vy = ((last.2 - first.2) / dt as f32).clamp(-1100.0, 600.0);
    // Upward bias so drops feel like tosses, not placements.
    if vy > -60.0 {
        vy = -60.0;
    }
    (vx, vy)
}

/// Desired gaze: direction to the target, normalised to -1..=1.
fn compute_gaze(cx: f32, cy: f32, mx: f32, my: f32) -> (f32, f32) {
    let dx = cx - mx;
    let dy = cy - my;
    let d = (dx * dx + dy * dy).sqrt().max(1.0);
    (
        (dx / d * (d / 280.0).min(1.0) * 2.0).clamp(-1.0, 1.0),
        (dy / d * (d / 280.0).min(1.0) * 2.0).clamp(-1.0, 1.0),
    )
}

fn spawn_point(
    monitors: &[mote_win::MonitorInfo],
    taskbar: &mote_win::TaskbarInfo,
    preferred: u32,
) -> (f32, f32) {
    let m = monitors
        .get(preferred as usize)
        .or_else(|| monitors.iter().find(|m| m.primary))
        .or_else(|| monitors.first());
    match m {
        Some(m) => {
            let x = (m.x + m.w / 2) as f32;
            // Prefer the taskbar top if it sits on this monitor.
            let y = if taskbar.y >= m.y && taskbar.y <= m.y + m.h {
                taskbar.y as f32
            } else {
                (m.y + m.h - 60) as f32
            };
            (x, y)
        }
        None => (960.0, 1000.0),
    }
}

fn strip_window_supports(world: &mut WorldSnapshot) {
    world
        .supports
        .retain(|s| s.kind != mote_core::SupportKind::Window && !s.is_wall());
}

/// Window procedure: routes to the App via GWLP_USERDATA.
pub unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        // lpCreateParams carries *mut App.
        #[repr(C)]
        struct CreateStruct {
            lp_create_params: *mut std::ffi::c_void,
            _rest: [usize; 10],
        }
        let cs = lparam.0 as *const CreateStruct;
        if !cs.is_null() {
            let app = (*cs).lp_create_params as isize;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, app);
        }
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let app = &mut *(ptr as *mut App);
    // WM_CREATE arrives before attach() sets app.hwnd — record it if needed.
    if msg == WM_CREATE && app.hwnd.0.is_null() {
        app.hwnd = hwnd;
    }
    match app.handle_message(hwnd, msg, wparam, lparam) {
        Some(r) => r,
        None => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throw_velocity_upward_bias() {
        let mut trail = VecDeque::new();
        trail.push_back((0.0, 100.0, 100.0));
        trail.push_back((0.05, 110.0, 100.0)); // nearly still
        let (vx, vy) = throw_velocity(&trail, 0.06);
        assert!(vy <= -60.0, "drops must toss slightly, got {vy}");
        let _ = vx;
    }

    #[test]
    fn gaze_clamped() {
        let (gx, gy) = compute_gaze(5000.0, 5000.0, 0.0, 0.0);
        assert!(gx <= 1.0 && gy <= 1.0);
        let (zx, zy) = compute_gaze(10.0, 0.0, 10.0, 0.0);
        assert!(zx.abs() < 0.01 && zy.abs() < 0.01);
    }

    #[test]
    fn strip_keeps_taskbar() {
        use mote_core::{Support, SupportKind};
        let mut w = WorldSnapshot::empty();
        w.supports.push(Support {
            id: 1,
            kind: SupportKind::Taskbar,
            x1: 0.0,
            x2: 100.0,
            y: 50.0,
            y_bottom: 50.0,
            monitor: 0,
            generation: 1,
            stable: true,
        });
        w.supports.push(Support {
            id: 2,
            kind: SupportKind::Window,
            x1: 0.0,
            x2: 100.0,
            y: 20.0,
            y_bottom: 20.0,
            monitor: 0,
            generation: 1,
            stable: true,
        });
        strip_window_supports(&mut w);
        assert_eq!(w.supports.len(), 1);
        assert_eq!(w.supports[0].kind, SupportKind::Taskbar);
    }

    #[test]
    fn explicit_lineup_resizes_and_preserves_selected_order() {
        let mut app = App::new(Settings::default());
        app.set_lineup(vec![SpeciesId::Owl, SpeciesId::Rabbit, SpeciesId::Cat]);
        assert_eq!(
            app.instances.iter().map(|i| i.species).collect::<Vec<_>>(),
            vec![SpeciesId::Owl, SpeciesId::Rabbit, SpeciesId::Cat]
        );
        app.set_lineup(vec![SpeciesId::Tortoise]);
        assert_eq!(app.instances.len(), 1);
        assert_eq!(app.instances[0].sim.species, SpeciesId::Tortoise);
        assert_eq!(app.settings.mote_count, 1);
    }

    #[test]
    fn startup_uses_saved_individual_pets() {
        let app = App::new(Settings {
            pets: vec![
                SpeciesId::Fox,
                SpeciesId::Owl,
                SpeciesId::Dog,
                SpeciesId::Rabbit,
            ],
            ..Default::default()
        });
        assert_eq!(
            app.instances.iter().map(|i| i.species).collect::<Vec<_>>(),
            app.settings.pets
        );
        assert_eq!(app.instances.len(), 4);
    }

    #[test]
    fn inter_pet_gaze_is_correct() {
        let (gx, gy) = compute_gaze(300.0, 200.0, 200.0, 200.0);
        assert!(gx > 0.5);
        assert!(gy.abs() < 0.01);
    }

    #[test]
    fn alpha_hit_testing_and_ignore_mouse_on_negative_monitor() {
        let mut app = App::new(Settings::default());
        let hwnd = HWND(0x1234 as *mut std::ffi::c_void);
        let inst = &mut app.instances[0];
        inst.overlay = Some(Overlay::dummy(hwnd));
        inst.presented_origin = (-200, -200);
        inst.presented_frame = vec![0; mote_render::SPRITE_PX * mote_render::SPRITE_PX * 4];
        inst.presented_frame[(100 * mote_render::SPRITE_PX + 100) * 4 + 3] = 255;
        let at = |x: i32, y: i32| LPARAM((((y as u16 as u32) << 16) | x as u16 as u32) as isize);
        assert_eq!(app.hit_test(hwnd, at(-100, -100)).0, HTCLIENT as isize);
        assert_eq!(app.hit_test(hwnd, at(-101, -100)).0, HTTRANSPARENT as isize);
        assert_eq!(app.hit_test(hwnd, at(-210, -210)).0, HTTRANSPARENT as isize);
        app.settings.click_through = true;
        assert_eq!(app.hit_test(hwnd, at(-100, -100)).0, HTTRANSPARENT as isize);
        let _ = app.instances[0].overlay.take().map(std::mem::forget);
    }
}
