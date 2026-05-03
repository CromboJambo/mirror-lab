# Crabjar Workspace
The agent's cognitive workspace for the Machine-Assisted Learning (MAL) loop. Pure observer layer — state-docs, overlays, knowledge store. Runtime execution disabled.

**Key Files:** `src/main.rs`, `state-docs/`, `memory/`, `reference_materials/`
**Dependencies:** `crabjar-config`, `agent-context`, `mirror-guard` (consumed by orchestrator)
**Data Source:** SQLite-backed memory and filesystem state-docs
**Capabilities:** State-docs read/write, overlay annotations, knowledge store queries (no command dispatch)
