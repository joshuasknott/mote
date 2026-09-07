# Mote build plan

This plan is intentionally vertical. A stage is complete only when the behaviour has been run and verified on a real Windows 11 installation, its important logic is tested, and the documentation describes the actual state accurately.

## Status

- **Implemented:** repository bootstrap, product brief, and this staged plan.
- **In progress:** none.
- **Remaining:** all application implementation stages below.

## Stages

### 0. Repository and architecture decision

- Choose and document the native/overlay architecture.
- Establish formatting, linting, type checking, tests, structured local logging, and error handling.
- Define boundaries between Windows sensing, desktop-world geometry, physics, behaviours, animation/rendering, persistence, and settings.
- Record the decision and its trade-offs in the README.

### 1. First living Mote: transparent overlay

- Launch one creature as a transparent, chrome-free, non-focus-stealing Windows overlay.
- Keep it lightweight and allow ordinary applications to remain usable.
- Add a minimal temporary renderer and a reliable update loop.
- Verify startup, shutdown, display changes, DPI changes, and recovery from overlay/API failures.

**Exit criterion:** one visible Mote can sit on the Windows desktop without behaving like a normal application window.

### 2. Desktop and taskbar geometry

- Detect monitor bounds, work areas, DPI, taskbar position/bounds, and auto-hide state.
- Track geometry changes rather than assuming a single monitor or fixed taskbar.
- Represent taskbar and screen-edge surfaces in a desktop-world model.

**Exit criterion:** Mote can identify and stay aligned with the taskbar and relevant screen surfaces across supported monitor layouts.

### 3. Deterministic physics

- Add position, velocity, gravity, collision, landing, edge detection, interruption, and invalid-state recovery.
- Keep the simulation separable from Windows APIs and deterministic enough for tests.
- Support dragging as an externally controlled physics interaction.

**Exit criterion:** Mote can fall, land, walk, jump, and recover predictably on the taskbar/screen surfaces.

### 4. Movement and animation

- Add walking, idle, sitting, falling, jumping, landing, sleeping, waking, stretching, and transition animations.
- Use proper frame timing, orientation/flipping, interpolation, easing, anticipation, squash/stretch, and small idle imperfections.
- Keep the renderer replaceable so polished final assets can replace temporary assets cleanly.

**Exit criterion:** Mote already feels alive when ignored for several minutes.

### 5. Window sensing and window surfaces

- Detect visible top-level windows and track movement, resizing, focus, minimisation, maximisation, and disappearance.
- Add window-top and allowed side-edge surfaces.
- React correctly when the underlying window moves or vanishes.

**Exit criterion:** Mote can stand on real application windows without faking the environment.

### 6. Climbing and traversal

- Add climbing, edge dangling, jumping between nearby windows, monitor movement, path/target surface selection, and graceful interruption.
- Handle taskbar auto-hide and monitor/device changes.

**Exit criterion:** Mote can traverse the supported desktop world and recover from topology changes.

### 7. Coherent behaviour system

- Implement a real state machine or utility-based system.
- Add mood/internal state such as energy, curiosity, boredom, comfort, excitement, sleepiness, and stress.
- Make decisions depend on personality, environment, recent actions, cooldowns, and weighted choices—not arbitrary rapid random switching.
- Implement idle, wandering, curious, startled, chasing/avoiding the cursor, recovery, and related states.

**Exit criterion:** behaviour remains varied but recognisably coherent over a long unattended session.

### 8. User interaction

- Add click, pet/stroke, drag-and-drop, contextual right-click actions, temporary sleep, hide, and call-back behaviour.
- Keep interaction optional and non-intrusive.
- Use click-through regions where appropriate while preserving deliberate creature interaction.

**Exit criterion:** interacting with Mote feels physically satisfying and never blocks normal computer use.

### 9. Idle, load, and fullscreen awareness

- Detect system idle/return, CPU usage, memory pressure where useful, screen lock, sleep/resume, and fullscreen applications.
- Let these signals influence behaviour and reduce update/animation cost appropriately.
- Add local development performance instrumentation; ship no telemetry.

**Exit criterion:** Mote becomes quiet or reactive appropriately without becoming an obligation or performance problem.

### 10. Media and music reactions

- Detect playback where practical and approximate local sound intensity without recording or retaining audio.
- Add subtle movement/bobbing, occasional dance, and natural stop behaviour.
- Keep all media/audio sensing local; never upload or store audio.

**Exit criterion:** music reactions feel like a delightful occasional response rather than a repetitive demo.

### 11. Tray, settings, and persistence

- Add a polished, secondary settings surface and tray controls.
- Support the minimal settings from the brief: startup, size, animation intensity, sound/music, cursor and CPU reactions, climbing, preferred monitor, reduce motion, and fullscreen pause.
- Persist settings with sensible defaults and recover safely from malformed state.

**Exit criterion:** core pet behaviour remains the focus while users can control interruptions and preferences.

### 12. Reliability, multi-monitor, and edge cases

- Verify monitor connect/disconnect, DPI changes, Explorer restart, taskbar changes, window disappearance, screen lock, sleep/resume, fullscreen apps, and unexpected API/process failures.
- Add crash recovery and clear local logs.
- Test multiple monitors and supported taskbar positions.

**Exit criterion:** Mote behaves gracefully during ordinary Windows lifecycle changes.

### 13. Final polish and packaging

- Replace temporary assets as appropriate, refine animation and behavioural coherence, optimise idle performance, and package a real Windows build.
- Document installation, supported Windows versions, known limitations, and exact implemented functionality.
- Review privacy: no account, analytics, telemetry, desktop/window uploads, or audio uploads.

**Exit criterion:** Mote feels like unusually alive, downloadable software rather than a proof of concept.

## Working rules

1. Work on one vertical stage at a time.
2. Run the application on Windows at every meaningful stage.
3. Add deterministic tests for physics and behaviour decisions wherever possible.
4. Keep platform-specific sensing behind clear interfaces.
5. Update this file as stages move from remaining to implemented.
6. Never mark a feature complete because it compiles or because a UI mock exists.
7. Preserve the future possibility of multiple Motes without adding networking now.
