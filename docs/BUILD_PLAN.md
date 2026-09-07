# Build plan — implemented vs remaining

Claims here are exact: anything listed as done was observed running on
Windows 11 (screenshots + behaviour log), not merely compiled.

## Done (vertical slice, all verified live)

- [x] Transparent overlay (`WS_EX_LAYERED`, per-pixel alpha, no focus steal,
      no taskbar/Alt-Tab presence, click-through except on the body via
      `WM_NCHITTEST`)
- [x] Desktop/taskbar geometry (virtual screen, per-monitor DPI, taskbar
      edge/rect/auto-hide, `TaskbarCreated`/Explorer-restart handling)
- [x] Physics (gravity, swept landing, support riding, support loss, edge
      falls, jump arcs, throws, out-of-world recovery) — 18 core tests
- [x] Window detection + standing on windows (filtered `EnumWindows`, DWM
      cloak, minimised exclusion, shell-chrome exclusion)
- [x] Jumping between windows (capability-checked targeting, anticipation/
      landing squash via spring)
- [x] Behaviour system (23-state utility machine, drives, personality,
      cooldowns, min durations; coherent multi-minute behaviour log)
- [x] Cursor interaction (gaze, startle on fast approach, chase, avoid,
      click-to-pet, drag-and-throw with trail velocity)
- [x] Idle/sleep (screensaver-grade idle source, sleep/wake poses, wake on return)
- [x] CPU/Memory reactions (Task-Manager-grade CPU deltas, wilt state)
- [x] Music reaction (peak-meter loudness, VU smoothing, playback hysteresis,
      bob + dance; zero audio capture by construction)
- [x] Tray icon (procedurally rendered) + full settings menu, persistence,
      launch-at-startup via HKCU Run key
- [x] Single instance, file logging with panic hook, `--self-test` smoke test
- [x] Suspend/resume, display-change rebuild, fullscreen-game auto-pause,
      DPI-awareness (PerMonitorV2)
- [x] CI (fmt/clippy/test/build), 49 tests, zero clippy warnings

## Remaining / known gaps (honest list)

1. **Wall-climbing animation** — `Climbing`/`Dangling` states exist and the
   brain can enter jump arcs to high ledges, but vertical wall-crawling with
   cling poses is simplified to a scramble-hop. Needs grip points + cling art.
2. **SMTC integration** — exact play/pause metadata from
   `Windows.Media.Control` is not wired; playback is inferred from sustained
   output levels (covers music/video/games uniformly, but can't distinguish
   them or read track state).
3. **Rich settings window** — tray menu covers all nine settings; a polished
   graphical panel is future work, not a gap in function.
4. **Multi-monitor soak** — virtual-screen model + per-monitor DPI are
   implemented and unit-covered, but long multi-monitor dogfooding (unplug
   during sleep, mixed-DPI drags) is still pending.
5. **Installer/packaging** — no MSIX/Inno installer yet; run the exe or
   `cargo install --path crates/mote-app`. Icon for the exe (vs tray) pending.
6. **Sprite art pass** — procedural renderer is deliberately swappable
   (`Animator`/`Pose` are asset-agnostic); final hand-drawn frames can drop in
   without touching sim or overlay code.
7. **Multiple Motes** — architecture is ready (`CreatureSim` is per-creature,
   world is shared); interaction senses + spawning UI not built.
8. **Reduced test coverage for overlay/tray** — message-loop code is verified
   live, not by unit tests (headless Win32 UI tests are brittle by nature).

## Deliberately out of scope (product principles)

Chat, LLMs, voice, notes/reminders, accounts, telemetry/analytics, network
access of any kind, Tamagotchi-style needs, dashboard-style settings app.
