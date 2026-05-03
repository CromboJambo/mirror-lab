# Mirror Guard Module
Trust layers, annealing, and execution gate — the authorization layer separating detection from action. Uses its own SQLite database (`guard.db`) distinct from `mirror-log`'s `mirror.db`.

**Key Files:** `Cargo.toml`, `src/`, `index.md`, `manifest.json`, `schema.sql`
**Dependencies:** `rusqlite`, `chrono`, `uuid`, `serde`, `serde_json`, `thiserror`
**Data Source:** `guard.db` — memory graph with nodes (facts, patterns, rules, reflections, outcomes, residues) and weighted directed edges
**Capabilities:** Trust scoring, annealing pipeline, execution gate (6 ordered checks), memory retrieval, confidence reinforcement/decay

**Modules:**
- `guard_db` — connection management, schema initialization
- `memory` — memory graph: nodes and weighted edges
- `trust` — TrustManager: layer lookups, confidence scoring
- `annealing` — AnnealingPipeline: iterative decay, reinforcement, action lifecycle
- `gate` — ExecutionGate: raw data ref, uncertainty, interruptibility, command risk
- `retrieval` — RetrievalEngine: band-based querying, stale detection
- `types` — shared types: TrustScore, MemoryNode, MemoryEdge, TrustLayer
**Test coverage:** 27 passing unit tests
