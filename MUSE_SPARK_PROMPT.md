Build a Windows-first desktop application called **Mote**.

Mote is the world's most overengineered desktop pet.

It is NOT an AI assistant, chatbot, productivity tool, or virtual companion that talks to the user. It is simply a small animated creature that genuinely appears to live inside Windows.

The goal is to make it delightful, technically impressive, polished, playful, and surprisingly alive.

## Core concept

Mote exists directly on the Windows desktop as a small animated creature.

It should be able to:

- sit and walk along the taskbar
- climb around the edges of application windows
- jump between nearby windows
- fall with gravity
- land on windows and the taskbar
- sleep when the computer is idle
- wake up when the user returns
- occasionally wander around on its own
- react when windows are moved underneath it
- react to mouse movement without constantly getting in the way
- become startled if the cursor approaches quickly
- occasionally chase the cursor
- get bored if nothing happens for a long time
- react when CPU usage becomes high
- react when the computer is under heavy load
- react to music playing on the computer
- subtly move/bounce to music intensity
- notice when media starts/stops
- have different moods and behaviours rather than looping one animation forever

The creature should feel like it understands the physical desktop environment.

## Important product principles

Mote should feel:

- charming
- slightly mischievous
- physically believable
- lightweight
- non-intrusive
- beautifully animated
- polished enough to feel like real downloadable software

Do NOT turn it into a dashboard-heavy application.

The pet itself is the product.

Settings should exist, but remain secondary.

Do NOT add chat, LLMs, prompts, AI assistants, productivity functionality, notes, reminders, or generic SaaS features.

## Windows integration

Target modern Windows 11 first.

Build genuine Windows integration rather than faking the environment visually.

Mote should understand things such as:

- desktop bounds
- multiple monitors
- DPI scaling
- taskbar position and bounds
- visible top-level windows
- window position and size
- windows moving/resizing
- minimised/maximised state
- active/focused window
- cursor position and velocity
- system idle time
- CPU usage
- memory pressure where useful
- media playback state
- system audio activity / approximate audio intensity where practical

Use appropriate native Windows APIs.

Avoid polling aggressively where event-driven approaches exist.

Mote must not interfere with ordinary computer use.

The overlay should:

- be transparent
- have no standard window chrome
- not appear as a normal application window during ordinary pet behaviour
- avoid stealing keyboard focus
- support click-through regions where appropriate
- allow deliberate interaction with the creature itself
- behave properly across multiple monitors
- handle DPI changes
- remain performant

## Physical desktop model

Treat the Windows desktop as a small 2D physical world.

Represent surfaces such as:

- top of taskbar
- top edges of ordinary windows
- side edges of windows where climbing is allowed
- screen edges
- optionally desktop floor when appropriate

Create a proper behaviour/physics layer rather than manually scripting every movement.

Include:

- position
- velocity
- gravity
- jumping
- falling
- collision
- landing
- edge detection
- climbing
- path selection
- target surfaces
- interruption when underlying windows move/disappear
- recovery if Mote gets into an invalid state

Keep the simulation deterministic enough to test.

Do not use a heavyweight game engine unless there is a genuinely strong technical reason.

## Behaviour system

Build Mote around a real behavioural state machine or utility-based behaviour system.

Possible behaviours:

- idle
- looking around
- walking
- running
- climbing
- jumping
- falling
- landing
- sitting
- sleeping
- waking
- stretching
- startled
- curious
- chasing cursor
- avoiding cursor
- dancing/bobbing to music
- reacting to high CPU usage
- examining an active window
- moving to another monitor
- dangling from an edge
- losing balance
- recovering
- playing with another Mote in the future

Mote should not switch behaviours randomly every few seconds.

Use personality, mood, environment, recent actions, cooldowns and weighted decisions so its behaviour feels coherent.

Maintain internal state such as:

- energy
- curiosity
- boredom
- comfort
- excitement
- sleepiness
- stress
- affection/familiarity if useful

These should influence actions but should not become a Tamagotchi-style obligation system.

The user should never need to keep Mote alive.

## Personality

Give Mote a recognisable personality through movement rather than dialogue.

Default personality:

- curious
- cheeky
- occasionally lazy
- energetic in short bursts
- likes music
- dislikes heavy CPU load
- sometimes watches what the user is doing
- sometimes completely ignores them

Avoid speech bubbles except perhaps extremely rare symbolic/emotive effects.

Prefer animation and behaviour over text.

## Visual direction

Start with a distinctive original creature.

Do not use copyrighted characters.

Do not make it look like a generic emoji, corporate mascot, Pokémon clone, or anime pet.

Aim for something simple enough to animate extensively but distinctive enough to become recognisable.

Think:

- compact silhouette
- expressive eyes/body language
- strong readable poses at small sizes
- soft but not childish
- clean enough to sit naturally over Windows

Use temporary programmatic/vector/sprite assets if necessary during engineering, but design the rendering pipeline so high-quality final animation assets can replace them cleanly.

Animation should support multiple states and transitions.

Prefer a proper sprite animation system with frame timing, orientation/flipping and interpolation where appropriate.

## User interaction

Interactions should include things like:

- click Mote
- pet/stroke interaction
- drag Mote and drop it somewhere
- Mote reacts differently depending on how it is dropped
- right-click opens a small contextual menu
- temporarily tell Mote to sleep
- temporarily hide Mote
- call Mote back if it wanders somewhere inconvenient

Dragging it around should feel physically satisfying.

Do not make the user constantly interact with it.

Mote should be enjoyable even when ignored.

## Music reaction

When media/music is playing:

- detect playback where possible
- detect approximate sound intensity without recording or retaining audio
- make Mote subtly react to rhythm/intensity
- occasionally enter a more explicit dance state
- stop naturally when playback ends

Do not upload or store audio.

Keep this local.

## Performance

This is a desktop toy and should behave like one.

Target:

- negligible CPU usage while idle
- modest memory usage
- no constant GPU abuse
- no interference with games or demanding applications
- automatically reduce animation/update frequency when appropriate
- pause/simplify expensive behaviour during fullscreen games where sensible

Create performance instrumentation for development.

Do not ship telemetry.

## Privacy

Mote should be local-first.

Do not require an account.

Do not send desktop/window/application information to any server.

Do not include analytics or telemetry.

Do not upload audio.

Avoid storing unnecessary information about which applications the user runs.

## Settings

Keep settings minimal and polished.

Possible settings:

- launch at startup
- creature size
- animation intensity
- sound reactions on/off
- music reactions on/off
- cursor interactions on/off
- CPU reactions on/off
- allow climbing application windows
- preferred monitor
- reduce motion
- pause while fullscreen applications are active

Provide a tray icon for basic controls.

The settings interface should not dominate the project.

## Architecture

Choose an architecture appropriate for a highly integrated Windows desktop application.

Strong preference:

- Rust for native/system/physics/process logic
- Windows APIs through `windows-rs`
- Tauri 2 and a small React/TypeScript interface if useful for settings
- native transparent overlay windows controlled from Rust

However, if another architecture is materially better for reliable transparent multi-monitor overlays, explain why in the repository documentation and use it.

Keep strict boundaries between:

- Windows environment sensing
- physical desktop world model
- creature simulation
- behaviour system
- animation/rendering
- persistence
- settings UI

Do not dump all behaviour into one giant file.

## Engineering quality

Treat this as real software, not a hackathon prototype.

Create:

- clean repository structure
- architecture documentation
- automated tests for physics/state/behaviour logic
- deterministic tests where possible
- linting
- formatting
- type checking
- error handling
- structured local logging
- crash recovery
- persistence for settings
- sensible configuration defaults

Handle:

- taskbar auto-hide
- multiple monitors
- different taskbar positions where Windows allows them
- DPI scaling
- monitors being connected/disconnected
- windows disappearing while Mote is standing on them
- Explorer restarting
- sleep/resume
- screen locking
- fullscreen apps
- unexpected process/API failures

Fail gracefully.

## Development workflow

Do not try to implement everything as superficial scaffolding.

Build vertically.

Start by making ONE Mote genuinely appear in Windows as a transparent overlay and interact correctly with the taskbar.

Then progressively add:

1. reliable transparent overlay
2. desktop/taskbar geometry
3. physics
4. walking and idle animation
5. window detection
6. standing on windows
7. jumping/climbing
8. behaviour system
9. cursor interaction
10. idle/sleep detection
11. system-load reactions
12. media/music reactions
13. settings/tray
14. multi-monitor robustness
15. animation polish
16. packaging
17. performance optimisation

At each stage, run the application and verify the behaviour rather than merely compiling it.

## Visual polish

Spend meaningful effort on:

- easing
- squash/stretch
- anticipation before jumps
- landing reaction
- tiny idle movements
- smooth transitions
- edge awareness
- believable acceleration/deceleration
- different sleep poses
- looking toward interesting things
- reacting to sudden movement
- small random imperfections in movement

The difference between a bad desktop pet and a brilliant one will be animation quality and behavioural coherence.

Prioritise those.

## Future-proofing

Architect the world so a future version could support multiple Motes on one desktop.

They might eventually:

- notice one another
- chase each other
- sleep together
- fight/play
- share window surfaces
- react to each other's movement

Do not build multiplayer/networking now.

Just avoid making the simulation inherently single-creature.

## Deliverable

Build the application in this repository.

Maintain a concise `README.md` explaining:

- what Mote is
- current implemented functionality
- how to run it
- architecture
- known limitations

Maintain a `docs/BUILD_PLAN.md` with implemented vs remaining work.

Keep claims exact. Do not describe mocked/scaffolded behaviour as working.

You have broad freedom to make good product and engineering decisions without repeatedly asking me.

When something is unclear, choose the option most consistent with:

**“a tiny creature genuinely lives inside Windows.”**

Do not stop at a basic proof of concept.

Once the core is functional, keep iterating on behaviour, animation, reliability, Windows integration, edge cases and polish until Mote feels unusually alive.