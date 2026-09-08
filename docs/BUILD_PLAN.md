# Build plan — implemented, verified, and remaining

## Character and interaction pass (8 September 2026)

Implemented in the application:

- Twelve individually authored cubic silhouettes replace the ellipse/triangle
  renderer: bent horns, continuous curls, floppy ears, a real ring opening,
  draped arms, and Kaiju dorsal plates and tail.
- Muted vision-board colours, ink contours, asymmetric eyes and brows,
  curved mouths and paws, irregular flank markings, and cached pigment grain.
- Foot-anchored squash/stretch, whole-body tilt, articulated steps, antenna
  sway (disabled by reduce motion), peeking fingers and climbing grips.
- Ring-tail has a separate curled nap drawing; other species use compressed
  sleeping poses. Entering/leaving the Ring-tail drawing still needs a morph.
- Oversized/stretching silhouettes fit the native canvas without cutting off
  horns. All three size settings have automated boundary coverage.
- Hit testing uses the last presented frame's alpha, including holes and
  negative monitor coordinates, instead of an invisible circular target.
- Reproducible runtime gallery, pose sheet, size sheet and renderer timing:
  `cargo run --release -p mote-render --example gallery`.

Verification in this pass:

- Windows workspace tests, format check and strict Clippy; release build.
- Native `mote.exe --self-test`: sensors, simulation, twelve renders and pack
  initialization pass. Its pose checks are render smoke checks, not proof of
  every autonomous behaviour on the desktop.
- Observed the new Climber on a real window edge in the Windows overlay.
- Inspected all twelve drawings on light/dark backgrounds and all three sizes.
- Release renderer measured approximately 0.8 ms/frame across 240 warm,
  mixed-species frames on this machine. This excludes sensing, simulation and
  native presentation; it is not a whole-app CPU or battery benchmark.

## Existing capabilities

These are present in source and covered by the current tests where practical;
this pass did not repeat every historical live acceptance check.

- Native transparent, non-activating layered overlays and tray controls.
- Desktop/taskbar/window geometry; gravity, swept landings, moving support
  riding/loss, window jumps and vertical wall climbing.
- Species personalities, drives, cooldowns, coherent behaviour selection,
  cursor gaze/startle/chase, petting, drag/throw, idle sleep and wake.
- Local CPU/memory sensing and output peak-meter music reaction; no recording.
- Settings persistence, startup option, single-instance guard, local logging,
  suspend/resume and fullscreen pause handling.
- One to four Motes with selectable species and shared world geometry.
- Windows CI runs format, Clippy, tests, native smoke and release build.

## Next improvements, in priority order

1. **Animation choreography:** richer species-specific walks, climb hand-over-
   hand cycles, distinct jump/landing poses, and smooth curled-sleep transitions.
   The current illustrations are animated vectors, not a full frame-by-frame
   hand-painted animation set.
2. **Desktop acceptance:** moving/minimising a climbed window, Explorer restart,
   lock/resume, autohide taskbars, and a four-pet soak with CPU/RSS measurements.
3. **Multi-monitor acceptance:** mixed DPI, negative coordinates, unplug/replug,
   and dragging between monitors. Unit coverage is not a substitute for hardware.
4. **Media state:** integrate SMTC for actual play/pause; the present output
   meter also responds to video and games and cannot identify music.
5. **Packaging:** installer and executable icon, signing/release workflow and
   install/uninstall checks. A release executable is available from Cargo.
6. **Social behaviour:** existing cohabitation/gaze is a starting point; richer
   shared play, resting together and collision avoidance remain.
7. **Settings polish:** a small optional native panel if the tray becomes
   cumbersome. Keep the creature as the product.

## Out of scope

Chat, LLMs, voice, notes/reminders, accounts, telemetry, network access,
Tamagotchi obligations, and dashboard-style settings.
