# Mote agent guide

Before changing code, read:

1. [MUSE_SPARK_PROMPT.md](./MUSE_SPARK_PROMPT.md)
2. [docs/BUILD_PLAN.md](./docs/BUILD_PLAN.md)
3. [README.md](./README.md)

Build Mote as a real Windows desktop application in vertical slices. Start with the first genuinely visible transparent overlay, then verify it on Windows before expanding the simulation.

Keep these constraints:

- Mote is a desktop pet, not an assistant, chatbot, productivity app, or generic SaaS product.
- Prefer native Windows integration and a clean boundary between platform sensing, world geometry, physics, behaviours, rendering, persistence, and settings.
- Do not claim mocked or scaffolded behaviour is working.
- Add tests for deterministic physics and behaviour logic.
- Keep logging local and useful; do not add telemetry or upload desktop/audio data.
- Update the README and build plan whenever implementation status changes.
- Make small, reviewable commits with clear messages.
