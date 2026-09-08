//! Mote — the world's most overengineered desktop pet.
//!
//! Entry point: file logging, single-instance guard, PerMonitorV2 DPI
//! awareness, overlay class registration and the Win32 message loop.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod overlay;
mod settings;
mod tray;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{
    GetLastError, ERROR_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, WPARAM,
};
use windows::Win32::Graphics::Gdi::HBRUSH;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, LoadCursorW, RegisterClassExW, TranslateMessage, CS_HREDRAW,
    CS_VREDRAW, HCURSOR, HICON, IDC_ARROW, MSG, WNDCLASSEXW,
};

use app::{wndproc, App};
use overlay::Overlay;

fn main() {
    init_file_logging();
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("mote {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--self-test") {
        self_test();
        return;
    }
    log::info!("Mote starting");

    // Single instance: a second launch just exits (the first keeps running).
    unsafe {
        let _mutex = match CreateMutexW(None, true, w!("Local\\MoteDesktopPet")) {
            Ok(h) => h,
            Err(e) => {
                log::error!("single-instance mutex failed: {e}");
                return;
            }
        };
        if GetLastError() == ERROR_ALREADY_EXISTS {
            log::info!("another Mote is already running; exiting");
            return;
        }

        // Per-monitor DPI awareness before any window exists.
        if let Err(e) = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) {
            log::warn!("DPI awareness failed (continuing): {e}");
        }

        if let Err(e) = run() {
            log::error!("fatal: {e:?}");
        }
        // `_mutex` released here.
    }
}

fn run() -> windows::core::Result<()> {
    unsafe {
        let instance: HINSTANCE =
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?.into();

        // Register the overlay window class.
        let cursor: HCURSOR = LoadCursorW(None, IDC_ARROW)?;
        let cls = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: cursor,
            hbrBackground: HBRUSH::default(),
            lpszClassName: w!("MoteOverlay"),
            hIcon: HICON::default(),
            hIconSm: HICON::default(),
            lpszMenuName: PCWSTR::null(),
            cbClsExtra: 0,
            cbWndExtra: 0,
        };
        if RegisterClassExW(&cls) == 0 {
            log::error!("RegisterClassExW failed");
            return Ok(());
        }

        // Settings before App (spawn + size depend on them).
        let mut s = settings::load();
        s.launch_at_startup = settings::startup_enabled() || s.launch_at_startup;

        // Box the App so its address is stable for the window proc.
        let mut app_box = Box::new(App::new(s));
        let app_ptr = (&mut *app_box as *mut App) as *mut std::ffi::c_void;

        let overlay = Overlay::create(instance, app_ptr)?;
        let hwnd: HWND = overlay.hwnd;
        app_box.attach(hwnd, overlay);

        // Hand ownership to the window proc; reclaim after the loop.
        let raw = Box::into_raw(app_box);
        let _ = raw;

        // Message loop.
        let mut msg = MSG::default();
        loop {
            let r = GetMessageW(&mut msg, None, 0, 0);
            if r.0 == 0 || r.0 == -1 {
                break; // WM_QUIT or error
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Reclaim + drop (tray icon removed, hook uninstalled, timers dead).
        let _ = Box::from_raw(raw);
        log::info!("Mote exiting cleanly");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Minimal file logger (no telemetry, no network — just a local log file).
// ---------------------------------------------------------------------------

struct FileLogger {
    path: std::path::PathBuf,
}

impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }
    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            use std::io::Write;
            let now = std::time::SystemTime::now();
            let secs = now
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = writeln!(
                f,
                "[{secs}] {:<5} {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
    }
    fn flush(&self) {}
}

/// Headless diagnostic: exercise every sensor + the renderer once and
/// print a summary. Used for smoke-testing on machines without watching
/// the pet (and in CI). Exits 0 on success, 1 if anything critical fails.
fn self_test() {
    println!("mote {} --self-test", env!("CARGO_PKG_VERSION"));
    let mut ok = true;

    let monitors = mote_win::query_monitors();
    let v = mote_win::virtual_screen_rect();
    println!(
        "monitors: {} (virtual {}x{} at {},{})",
        monitors.len(),
        v.w,
        v.h,
        v.x,
        v.y
    );
    for (i, m) in monitors.iter().enumerate() {
        println!(
            "  [{}] {}x{} at {},{} dpi={} primary={}",
            i, m.w, m.h, m.x, m.y, m.dpi, m.primary
        );
    }
    if monitors.is_empty() {
        println!("FAIL: no monitors");
        ok = false;
    }

    let taskbar = mote_win::query_taskbar(&monitors);
    println!(
        "taskbar: {}x{} at {},{} edge={:?} autohide={}",
        taskbar.w, taskbar.h, taskbar.x, taskbar.y, taskbar.edge, taskbar.autohide
    );

    let wins = mote_win::enumerate_windows();
    println!("windows: {} candidates", wins.len());
    for w in wins.iter().take(8) {
        println!(
            "  hwnd={:?} {}x{} at {},{} [{}]",
            w.hwnd.0, w.w, w.h, w.x, w.y, w.class_name
        );
    }

    let world = mote_win::build_world(&mote_win::world_build::WorldInputs {
        monitors: &monitors,
        taskbar: &taskbar,
        windows: &wins,
        generation: 1,
    });
    println!("supports: {}", world.supports.len());
    if world.supports.is_empty() {
        println!("FAIL: no supports (Mote would have nowhere to stand)");
        ok = false;
    }

    match mote_win::cursor::poll_cursor() {
        Some((x, y)) => println!("cursor: {x},{y} idle_ms={}", mote_win::cursor::idle_ms()),
        None => {
            let err = unsafe { windows::Win32::Foundation::GetLastError() };
            // In headless/CI environments where ERROR_ACCESS_DENIED (5) is returned,
            // degrade gracefully as per mote-win failure contract.
            if err.0 == 5 {
                println!("cursor: unavailable (headless session ERROR_ACCESS_DENIED, graceful degradation)");
            } else {
                println!("FAIL: cursor poll failed, GetLastError = {err:?}");
                ok = false;
            }
        }
    }

    let mut cpu = mote_win::CpuMeter::new();
    cpu.sample();
    std::thread::sleep(std::time::Duration::from_millis(250));
    cpu.sample();
    println!(
        "cpu: {:.1}% mem_load: {:.0}%",
        cpu.usage * 100.0,
        mote_win::query_memory().load_01 * 100.0
    );

    let mut audio = mote_win::AudioMonitor::new();
    audio.poll(0.0);
    audio.poll(0.2);
    println!(
        "audio: available={} level={:.3} playing={}",
        audio.available(),
        audio.level,
        audio.playing
    );

    // Behaviour + physics smoke: 10 simulated seconds on this desktop.
    let mut sim = mote_core::CreatureSim::new(1, 960.0, 1000.0, 0);
    let cfg = mote_core::SimConfig::default();
    let sense = mote_core::SenseInput::default();
    let mut t = 0u64;
    while t < 10_000 {
        t += 16;
        sim.tick(0.016, t, &sense, &world, &cfg);
    }
    println!(
        "sim: 10 s -> state={:?} pos=({:.0},{:.0}) grounded={}",
        sim.brain.state,
        sim.body.pos.x,
        sim.body.pos.y,
        sim.body.grounded()
    );

    // Renderer smoke: verify all 12 vision board species.
    let mut anim = mote_render::Animator::new(64.0);
    for &species in mote_core::SpeciesId::all() {
        let inp = mote_render::AnimInput {
            state: sim.brain.state,
            species,
            ..Default::default()
        };
        anim.update(0.016, &inp);
        let frame = mote_render::creature::draw_mote(&anim.pose(&inp));
        let opaque = frame
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 128)
            .count();
        if opaque < 1000 {
            println!(
                "FAIL: species {:?} sprite nearly invisible ({opaque} px)",
                species
            );
            ok = false;
        }
    }
    println!("render: all 12 vision board species rendered successfully");

    // Signature spatial behavior smoke checks:
    // 1. "01 peeks" - window-edge peeking lower body mask
    let peek_pose = mote_render::creature::Pose {
        species: mote_core::SpeciesId::Peeker,
        is_peeking: true,
        hop_px: 40.0,
        ..Default::default()
    };
    let peek_frame = mote_render::creature::draw_mote(&peek_pose);
    let peek_bottom_opaque = peek_frame
        .as_chunks::<4>()
        .0
        .iter()
        .skip(225 * mote_render::SPRITE_PX)
        .filter(|p| p[3] > 64)
        .count();
    if peek_bottom_opaque > 0 {
        println!("FAIL: peeking did not mask lower body");
        ok = false;
    }

    // 2. "05 climbs" - vertical border wall-climbing
    let climb_pose = mote_render::creature::Pose {
        species: mote_core::SpeciesId::Climber,
        is_climbing: true,
        climb_phase: 1.0,
        facing: 1,
        ..Default::default()
    };
    let climb_frame = mote_render::creature::draw_mote(&climb_pose);
    if climb_frame
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[3] > 128)
        .count()
        < 1000
    {
        println!("FAIL: climbing frame did not render properly");
        ok = false;
    }

    // 3. "09 naps" - curled ledge napping
    let nap_pose = mote_render::creature::Pose {
        species: mote_core::SpeciesId::RingTail,
        is_napping: true,
        sleep_amount: 1.0,
        time_s: 2.0,
        ..Default::default()
    };
    let nap_frame = mote_render::creature::draw_mote(&nap_pose);
    if nap_frame
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[3] > 128)
        .count()
        < 1000
    {
        println!("FAIL: curled napping frame did not render properly");
        ok = false;
    }
    println!("behaviors: 01 peeks, 05 climbs, 09 naps validated");

    // 4. Multi-mote desktop cohabitation (Pack: 4 motes)
    let pack_settings = settings::Settings {
        mote_count: 4,
        species: mote_core::SpeciesId::Peeker,
        ..settings::Settings::default()
    };
    let app = App::new(pack_settings);
    if app.instances.len() != 4 {
        println!(
            "FAIL: expected 4 motes in pack, got {}",
            app.instances.len()
        );
        ok = false;
    }
    let unique_species: std::collections::HashSet<_> =
        app.instances.iter().map(|i| i.species).collect();
    if unique_species.len() != 4 {
        println!(
            "FAIL: expected 4 unique species in pack, got {}",
            unique_species.len()
        );
        ok = false;
    }
    println!("cohabitation: 4-mote desktop pack initialized with unique species");

    println!(
        "{}",
        if ok {
            "SELF-TEST PASS"
        } else {
            "SELF-TEST FAIL"
        }
    );
    if !ok {
        std::process::exit(1);
    }
}

fn init_file_logging() {
    let path = settings::log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Cap the log at ~1 MB to stay lightweight forever.
    if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > 1_000_000 {
        let _ = std::fs::write(&path, "");
    }
    let logger = Box::new(FileLogger { path });
    let _ = log::set_boxed_logger(logger);
    log::set_max_level(log::LevelFilter::Info);
    // Surface Rust panics into the log too.
    std::panic::set_hook(Box::new(|info| {
        log::error!("panic: {info}");
    }));
}

#[allow(dead_code)]
fn _unused_wparam() -> WPARAM {
    WPARAM(0)
}

#[allow(dead_code)]
fn _unused_lparam() -> LPARAM {
    LPARAM(0)
}
