# Architecture

## Native Rust application

Mote uses four Rust crates and native Win32 windows. A 256x256 layered window
per animal handles transparent presentation without a browser or game engine.
A separate character picker uses GDI text and cached realistic pet portraits.
The picker is a top-level window: it must not be owned by an overlay because
hiding that overlay would also hide its owned windows.

| Crate | Responsibility |
|---|---|
| mote-core | deterministic physics, geometry, animal registry, drives and decisions |
| mote-win | monitor, taskbar, window, cursor, idle, system-load and audio sensing |
| mote-render | embedded PNG atlases, pose selection, transitions and premultiplied pixels |
| mote-app | native overlay/picker/tray, message loop, persistence and startup integration |

## Update and rendering

The active tick is 32ms. It polls cursor/idle, gates reactions through user
settings, refreshes native geometry on WinEvents or a two-second recovery
interval, advances each simulation, updates animation and presents frames.
CPU/fullscreen sensing runs at about 1Hz; audio sampling at about 10Hz only
when enabled. All-sleeping packs use a 64ms tick and render every second tick.
Hidden, picker-open and fullscreen-paused states use 250ms ticks and skip
simulation and rendering. The slow timer remains available during pause.

The desktop world contains taskbar/floor, window tops and optional wall
supports. Stable support IDs let physics ride moving windows or fall safely
when support disappears. Species motion traits constrain utility choices:
tortoises cannot voluntarily jump, climb, run or chase; rabbits and owls hop.
Physics friction is time-based and jump integration preserves launch velocity.

`Overlay::present` alone controls overlay position. `set_visible` never moves
it. The last successful premultiplied frame and origin remain the source of
truth for alpha hit testing. Explicit click-through uses WS_EX_TRANSPARENT
so input also passes to windows owned by other processes. No overlay steals
keyboard focus; the user opens the picker deliberately.

## Artwork

Each animal has an embedded transparent PNG with four columns and two rows:
stand, walk contact, walk passing, opposite contact, sit, sleep, anticipation,
leap/stretch. The renderer caches decoded frames and removes detached cell
spill. Source pose bounds supply foot anchors; rendering produces 256x256
premultiplied RGBA, which the overlay converts to BGRA for UpdateLayeredWindow.
The picker uses the same renderer for portraits. No asset lookup depends on
the checkout, current directory or network. Source prompts live beside the
atlases in assets/pets/README.md.

The animation is a finite raster pose system with procedural timing and
movement, not skeletal 3D animation. Validation covers transparency, bounds,
frame selection, transitions and deterministic playback. The gallery exposes
all species and poses for visual checks as well as renderer-only timing.

## Selection and persistence

The picker owns its state and posts WM_APP_PICKER_RESULT to the app. It never
borrows App or starts a nested message loop. Commit returns the selected
pets and Quiet/Reduce Motion settings; cancellation leaves saved settings
unchanged. The app keeps one to four simulations and a shared world snapshot.

Settings retain legacy species/count fields for migration, plus an explicit
bounded, deduplicated pets list. Former creature IDs deserialize through
aliases to real species. Startup registration uses --background; a deliberate
second launch posts WM_APP_OPEN_PICKER to the existing instance. Ctrl+Alt+M
uses a registered native hotkey, with tray fallback if another app owns it.

## Privacy and verification

No network stack, account, telemetry or audio recording is part of Mote.
Window information is transient geometry. Local logs reset on startup when
they exceed 1MB.
`MOTE_DATA_DIR` isolates settings and logs during acceptance runs.

Core and animation take explicit time and deterministic random streams.
Tests cover capability constraints, jump arcs, moving supports, friction,
settings migration, lineup ordering and renderer invariants. Sensor tests
cover signal maths and native API failure tolerance. --self-test exercises
live sensors plus simulation/rendering, but does not establish hardware
acceptance for mixed DPI, lock/resume, Explorer restart or monitor hotplug.
