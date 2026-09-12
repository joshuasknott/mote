# Build plan — implementation and acceptance

## Realistic pet release (12 September 2026)

Implemented:

- Six real animal species replace the fantasy roster: tabby cat, Shiba Inu,
  lop rabbit, red fox, barn owl and Hermann's tortoise.
- Six embedded transparent eight-pose atlases; native runtime rendering,
  species-consistent scaling, foot anchoring, walking poses, sleep and jump
  transitions, restrained breathing and landing flex. Tortoise shells remain
  rigid. Reduce Motion suppresses decorative movement.
- A native game-style character picker with large previews, a six-animal
  roster, explicit one-to-four-pet lineup, Quiet and Reduce Motion controls,
  mouse selection, keyboard focus traversal, and cancellation.
- Exact lineup persistence; old fantasy species deserialize to real animals.
  Settings writes replace the saved document after the new file is written.
- Small, taskbar-only quiet defaults. Cursor play, window climbing, CPU and
  audio reactions are optional. Click-through uses the native cross-process
  window style; Ctrl+Alt+M hides/shows with tray fallback.
- Normal and second launches reopen selection; --background restores the
  saved pets without showing the picker. Optional Windows startup uses this
  quiet launch path. --quit closes the running process cleanly.
- Species movement constraints: tortoises stay grounded, rabbits and owls
  hop, and non-running animals walk when called. Cursor chasing requires
  active user input.
- Corrected time-based friction, preserved airborne jump velocity, and
  horizontal support riding when a window moves.
- About 30fps active rendering, reduced sleep cadence, and paused simulation
  and rendering while hidden, choosing pets or in fullscreen applications.
- Native media-session play/pause information with local peak-meter fallback
  for applications that do not expose a media session. No audio recording,
  media titles, application identifiers, networking or telemetry.
- Portable ZIP packaging with all artwork embedded in Mote.exe, a quick-start
  guide and the license. No checkout or external asset files required.

## Verification record

Verified locally on Windows on 12 September 2026:

- `cargo fmt --all -- --check` and strict workspace/all-target Clippy passed.
- All 79 workspace tests passed: 15 app, 35 core, 10 renderer and 19 Windows.
  These include species movement limits, moving supports, jump velocity,
  settings migration, exact lineups, alpha blending and rotated sprite bounds.
- The native picker was inspected at 1040x680. Mouse selection, the four-pet
  limit, saved lineup order and Reduce Motion persistence were checked. The
  portable build also passed live arrow/Space selection and Escape cancellation
  checks, including preservation of the saved lineup after cancellation.
- Layout tests cover 1040x680, 900x600 and 720x500. The two smaller sizes were
  checked geometrically; they were not separate display-hardware observations.
- Release runtime galleries show all six animals, eight states per animal and
  all three sizes. The current gallery and native picker screenshot are in docs.
- The packaged executable passed `--self-test` from its own package directory:
  one 3440x1440 monitor at 96 DPI, bottom taskbar, live cursor/system/audio
  queries, simulation, all six renderers and four-species pack initialization.
  Native overlay logs also showed matching source/DIB alpha and running pet
  state transitions. This is API/log evidence, not a complete desktop soak.
- The portable ZIP contains exactly Mote.exe, LICENSE and QUICKSTART.md; its
  executable hash matches the release build. Artwork is embedded. The ZIP is
  about 11.4 MiB and the executable about 11.8 MiB.

The release gallery measured 9.287ms per warm mixed-species frame during a
concurrent build workload. That measurement excludes the rest of the app and
does not establish sustained whole-app frame rate or idle resource use.
Live media play/pause transitions were not exercised with a media player;
SMTC status handling is covered by focused tests.

## Acceptance boundaries

The animals use a finite set of generated realistic 2D poses. They are not
3D skeletal animals; independent continuous head/limb articulation and full
flight are outside this build. Window climbing reuses locomotion poses.

The code supports multiple monitors, DPI changes, support loss, fullscreen
pause and suspend/resume. Mixed-DPI monitor hotplug, Explorer restart,
lock/resume, auto-hide taskbars and long-running four-pet performance still
need broader hardware acceptance. Unit tests and the native smoke command
are not substitutes for those observations.

The portable Windows build is unsigned. A signed installer and public
release require a signing identity and separate publication authorization.

## Out of scope

Chat, LLMs, voice, notes/reminders, accounts, telemetry, networking, feeding
obligations, multiplayer and productivity dashboards.
