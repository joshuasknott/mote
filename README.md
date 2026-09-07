# Mote

Mote is the world's most overengineered desktop pet: a small animated creature that genuinely lives inside Windows.

It is a playful Windows-first desktop toy—not an AI assistant, chatbot, productivity tool, or virtual companion that talks to the user. The product is the creature itself: its movement, physical relationship with the desktop, moods, reactions, and animation quality.

## Current state

This repository has been bootstrapped for implementation. It currently contains:

- the authoritative Muse Spark 1.3 build brief in [MUSE_SPARK_PROMPT.md](./MUSE_SPARK_PROMPT.md)
- the staged implementation plan in [docs/BUILD_PLAN.md](./docs/BUILD_PLAN.md)
- repository guidance for OpenCode agents in [AGENTS.md](./AGENTS.md)

There is not yet a working desktop binary in this repository. Claims about implemented behaviour should be added here only after it has been run and verified on Windows.

## Development target

- Windows 11 first
- local-first and privacy-preserving
- transparent, non-intrusive multi-monitor overlay
- lightweight while idle
- native desktop integration
- behaviour and animation quality over dashboard features

The initial architecture decision should be documented once the first vertical slice is implemented. Rust with native Windows APIs and Tauri 2 is the preferred starting direction, but the implementation may choose another approach if it is materially better for reliable transparent overlays.

## Starting in OpenCode

From a PowerShell terminal:

```powershell
cd "$HOME\Projects\mote"
git pull --ff-only
```

Then give Muse Spark 1.3 the contents of [MUSE_SPARK_PROMPT.md](./MUSE_SPARK_PROMPT.md). Ask it to work directly in this repository, build vertically, run each stage on Windows, and keep the README and build plan exact.

If the repository has not been cloned yet:

```powershell
cd "$HOME\Projects"
git clone https://github.com/joshuasknott/mote.git
cd .\mote
```

## Principles

- The pet itself is the product.
- Do not add chat, LLMs, generic SaaS features, or a dashboard-heavy experience.
- Do not describe mocks, scaffolding, or plans as working functionality.
- Prefer deterministic, testable simulation and behaviour logic.
- Keep all desktop, window, cursor, and audio sensing local.
