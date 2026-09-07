# Architecture

## Decision: pure Rust + Win32, no Tauri, no game engine

The brief expresses a preference for Rust + `windows-rs` + Tauri 2, but
allows a better architecture with a written reason. We chose **pure Rust +
raw Win32**:

- The entire product surface is one 256×256 layered window updated at
  ~30 fps plus a tray menu. A Chromium/webview runtime (100 MB+, its own GPU
  process) would dwarf the pet to serve a settings menu with nine toggles.
- Transparent click-through-except-on-the-pet hit testing, per-pixel alpha,
  and `UpdateLayeredWindow` positioning are all one native call each. In a
  webview shell they would be harder, not easier.
- Idle cost today: one 16 ms timer, a software raster of 65k pixels at
  30 fps awake / 7 fps asleep, `EnumWindows` only on WinEvent notification
  (+2 s fallback). Measured ~15 MB RSS. A game engine would add nothing: the
  "physics" is a 2D platformer-lite (gravity, supports, jump arcs) in ~300
  lines.

If a future settings UI outgrows a tray menu, a small Tauri window can be
added without touching `mote-core`/`mote-win`/`mote-render`.

## Crate map

```text
mote-app      Win32 message loop, 16 ms + 1 s timers, wiring
  ├── overlay.rs   WS_EX_LAYERED window, DIB section, UpdateLayeredWindow blits
  ├── tray.rs      tray icon (procedurally rendered) + settings menu
  ├── settings.rs  %APPDATA% persistence + HKCU Run key
  └── app.rs       per-tick: sense → sim → animate → render → present
mote-win      environment sensing → plain data (all failures degrade)
  ├── monitors.rs  EnumDisplayMonitors + GetDpiForMonitor + virtual screen
  ├── taskbar.rs   SHAppBarMessage(ABM_GETTASKBARPOS/GETSTATE) + tray wnd rect
  ├── windows.rs   EnumWindows filter (visible, !minimised, !cloaked, …)
  ├── cursor.rs    smoothed velocity tracker + GetLastInputInfo idle
  ├── stats.rs     GetSystemTimes CPU deltas + GlobalMemoryStatusEx
  ├── audio.rs     IAudioMeterInformation peak + playback hysteresis
  ├── events.rs    SetWinEventHook → atomic generation counter
  └── world_build.rs  monitors+taskbar+windows → WorldSnapshot
mote-core     no OS calls; deterministic; unit-tested
  ├── physics.rs   bodies, gravity, swept landing, support riding/loss, jumps
  ├── world.rs     supports, queries, jump targeting, edge info
  ├── behaviour.rs Brain: utility-weighted state machine + cooldowns
  ├── personality.rs drives + traits (no obligations, never dies)
  └── lib.rs       CreatureSim::tick — the per-creature step
mote-render   asset-agnostic animation backend (procedural today)
  ├── anim.rs      Animator: squash spring, blinks, saccades, walk/dance phase
  └── creature.rs  software raster → premultiplied RGBA sprite
```

## Key flows

**One tick (16 ms):** poll cursor → tracker; idle; audio @10 Hz; CPU/mem/
fullscreen @1 Hz; rebuild world if WinEvents fired or 2 s elapsed; gate
senses by settings; `CreatureSim::tick` (drives → brain intent → physics);
animator update → pose → raster @30 fps awake / 7 fps asleep; `present(x,y)`
positions + blits the overlay in one call.

**Positioning authority:** `Overlay::present(x, y)` is the *only* code that
moves the window. (`set_visible(true)` uses `SWP_NOMOVE`.) This invariant
exists because a regression once parked the window at 0,0 via a
`SetWindowPos(0,0)` hidden inside the show path — see log sentinel
`UpdateLayeredWindow failed` (warn-once).

**Support invalidation:** physics validates `grounded_surface` every tick
(moved → ride it; gone → `SupportLost` → fall). World rebuilds bump
`generation`; supports use HWND-derived ids (taskbar/floor in reserved
ranges that can't collide).

**Event-driven where it matters:** `SetWinEventHook` (object
destroy→location-change range, minimise range, foreground) bumps an atomic;
the loop rebuilds lazily. Polling remains for cursor (16 ms, cheap),
audio (10 Hz), CPU (1 Hz), world fallback (2 s).

## Determinism & tests

`mote-core` and animation take no wall-clock input — time arrives as `dt` /
`now_ms` parameters, randomness from owned xorshift streams. Tests assert
repeatability (`run() == run()`), landing/edge/support-loss physics,
no-flap behaviour budgets, render premultiplication invariants, and sensor
math (CPU ratio, cursor smoothing, audio hysteresis). OS-touching tests only
assert crash-freedom and sanity on live Windows.

## Multi-Mote future

`CreatureSim` is already per-creature state stepped against a shared
`&WorldSnapshot`; `tick()` takes `&mut self` with no globals. A future
version can hold `Vec<CreatureSim>`, add inter-creature senses to
`SenseInput`, and share supports — no architectural change needed.
