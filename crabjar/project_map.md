# project_map.md

> Generated: May 03 2026
> Source: Cargo.toml (root + all members), README.md, AGENTS.md, filesystem scan, mirror-guard source
> Purpose: Structural alignment reference for agent navigation

---

## 1. Overview

mirror-lab is a Rust workspace for a personal knowledge-management system. Ingests filesystem events, stores them in an append-only SQLite log, supports semantic chunking and local AI querying, provides a voice/TTS interface.

---

## 2. Architecture

### 2.1 Workspace Layout

```
mirror-lab/
├── Cargo.toml               # Workspace root — 18 members, shared deps, release profiles
├── Cargo.lock               # Locked dependency graph (single monorepo lock)
├── LICENSE                  # MIT OR Apache-2.0 (workspace-level)
├── AGENTS.md               # Repository guidelines and architectural constraints
├── README.md               # Project overview and integration roadmap
├── mirror.db                # Primary append-only SQLite event log
│
│  Core mirror-* crates
├── mirror-log/              # Append-only event log, chunking, embeddings (v0.1.9, AGPL-3.0)
├── mirror-kernel/           # Decision logic, SQLite persistence, WIT interface consumer
│   └── mirror-voice/        # TTS interface (piper-tts sub-workspace)
├── mirror-daemon/           # File watcher + pipeline execution (Action Layer, Gated)
├── mirror-guard/            # Trust layers, annealing, execution gate (authorization layer)
├── mirror-logger/           # Structured logging engine and entry management
├── mirror-query/            # Local AI query CLI (decompression layer over mirror-log)
├── mirror-wit/              # WIT interface definitions + proc-macro companion
│   └── macro/               # Proc-macro crate for WIT codegen
├── mirror-ledger/           # Ledger artifacts, reflections, work directory (non-crate)
│
│  Supporting crates
├── zllg/                    # Zellij IDE orchestration layer (AGPL-3.0)
│
│  Crabjar — agent scratchpad
├── crabjar/                 # Observer layer — state-docs, overlays, knowledge store
│   ├── orchestrator/        # ACP-compliant orchestrator (consumes mirror-guard)
│   └── src/                 # codeburn crates, crabjar-config, knowledge_store, state_docs
│
├── staging/                 # Ephemeral JSON artifacts (1 staged artifact)
├── pipelines/               # Empty — future pipeline definitions
└── state-docs/              # Durable Markdown state documentation
```

### 2.2 Core Components

| Component | Role | Layer | Status |
| :--- | :--- | :--- | :--- |
| `mirror-log` | Append-only event log, chunking, embeddings | Detection (Read-Only) | Core, v0.1.9 |
| `mirror-guard` | Trust layers, annealing, execution gate | Authorization (Separate DB) | Core |
| `mirror-kernel` | Decision records, kernel dispatch | Decision (No Action) | Core |
| `mirror-daemon` | File watcher + pipeline execution | Action (Gated) | Core |
| `mirror-query` | Local AI query CLI | Query | Core |
| `mirror-logger` | Structured logging engine | Logging | Core |
| `mirror-wit` | WIT interface + proc-macro | Interface | Core |
| `zllg` | Zellij IDE orchestration | Tooling | Active |
| `crabjar` | Agent scratchpad, state-docs, overlays | Observer | Active |
| `crabjar/orchestrator` | ACP orchestrator with mirror-guard | Orchestrator | Active |
| `crabjar/src/codeburn-*` | Codeburn pipeline crates (5 crates) | Experimental | Active |

### 2.3 Workspace Members

Declared in Cargo.toml `[workspace.members]`:
- mirror-wit
- mirror-wit/macro
- mirror-daemon
- mirror-kernel
- mirror-logger
- mirror-query
- mirror-log
- crabjar/orchestrator
- mirror-guard
- crabjar/src/codeburn
- crabjar/src/codeburn-config
- crabjar/src/codeburn-provider
- crabjar/src/codeburn-classifier
- crabjar/src/codeburn-pricing
- zllg

### 2.4 Shared Dependencies

Declared in Cargo.toml `[workspace.dependencies]`:
- rustls-webpki (0.103.13 — RUSTSEC-2026 fix)
- serde (1.0, derive)
- serde_json (1.0)
- chrono (0.4, serde)
- tracing (0.1)
- tracing-subscriber (0.3, env-filter)
- uuid (1.10, v4)
- sha2 (0.10)
- hex (0.4)
- crossterm (0.28)
- rand (0.9)
- ratatui (0.30)
- clap (4.5, derive)
- notify (6.1)
- rusqlite (0.32, bundled)
- toml (0.8)
- regex (1.10)
- anyhow (1.0)
- thiserror (2.0)
- async-trait (0.1)
- tempfile (3.14)
- tokio (1.35, full)
- reqwest (0.12)
- ignore (0.4)
- path-absolutize (3.1)

### 2.5 Release Profile

opt-level = 3, lto = true

### 2.6 Dev Profile

debug = true

---

## 3. Build & Test

| Command | Purpose |
|---|---|
| `cargo build` | Debug build of entire workspace |
| `cargo build --release` | Optimised release build |
| `cargo check --workspace` | Fast type/borrow-check |
| `cargo test --workspace` | Run all tests |
| `cargo test -p <crate>` | Run tests for single crate |
| `cargo clippy --workspace -- -D warnings` | Lint; warnings treated as errors |
| `cargo fmt --all` | Auto-format every crate |
| `cargo fmt --all -- --check` | CI formatting gate |

mirror-log exposes optional features: `--features embedding` or `--features inference`.

---

## 4. Code Quality & Style

- Formatter: rustfmt with default settings
- Linter: Clippy at --deny warnings
- Naming: snake_case for functions/variables/modules, PascalCase for types/traits, SCREAMING_SNAKE_CASE for constants
- Error handling: thiserror for library crates, anyhow for binary/CLI crates
- No unwrap/expect for recoverable errors in library code
- Dependencies: add to workspace root first, then reference with { workspace = true }

---

## 5. Testing Guidelines

- Framework: Rust built-in #[test] and #[cfg(test)]
- SQLite tests: in-memory database (:memory:) or tempfile managed path
- Filesystem tests: use tempfile
- Test naming: descriptive snake_case stating behaviour under test
- Coverage: aim to cover full public API surface of each crate

---

## 6. Integration Roadmap

### Phase 1 — Standardization

align shared dependencies at workspace root; enforce unified error-handling patterns; ensure CI passes across all members

### Phase 2 — Feature Integration

move ingress logic into mirror-daemon; integrate clipboard-watching into daemon; converge CLI tooling into mirror-query

### Phase 3 — Consolidation

move completed experiments to archive/ directory; produce clean pre-optimized workspace

---

## 7. Architectural Constraints

### Truth vs Convenience

Detection ≠ authorization. Knowing what happened does not grant the right to change what happens.

### Detection vs Action Layer Separation

| Component | Role | Can act? |
|---|---|---|
| crabjar | Pure observer — state-docs, overlays, knowledge store | No |
| mirror-log | Append-only event log — no deletion, no modification | No |
| mirror-kernel | Decision records, kernel dispatch | No |
| mirror-daemon | File watcher + pipeline execution | Gated |

### Execution Gate (mirror-daemon)

Before any pipeline execution:
1. Raw data reference
2. Uncertainty exposure
3. Interruptibility

### Confidence Decay

Patterns decay once conditions change. Confidence decreases over time unless reinforced by recent success.

### Every Abstraction Carries Its Own Doubt

Every derived output must include: what it might have missed, what assumptions it made, where it might break, how stale it is.

---

## 8. Crabjar Context

### 8.1 Structure

crabjar contains:
- agent_config.md
- AGENTS.md
- Cargo.toml (workspace root + crabjar binary manifest)
- Justfile (task runner shortcuts)
- Containerfile, Dockerfile (container build definitions)
- orchestrator (Axum SSE server)
- mirror-guard (SecurityGuard)
- codeburn-config (config struct, TOML parsing)
- codeburn-provider (ProviderRegistry)
- codeburn-classifier (TaskClassifier)
- codeburn-pricing (PricingEngine)
- codeburn (CLI binary)
- memory/files (agent-context crate)
- tests/cli.rs
- ui-state-copy
- reference_materials
- bin/ (compiled binaries)
- git/ (git helper scripts)
- gitignore/ (gitignore management)
- workspace/ (workspace config)
- state-docs/ (local state-docs)
- src/models/ (model definitions)
- src/state-docs/ (state-docs source)
- src/dotfile_manager.rs (dotfile management)
- *.manifest.json (file manifests)
- human_reference.md (human reference documentation)
- environment_manifest.json (environment manifest)

**Removed items:**
- `js-code-sandbox/` — TypeScript LM Studio plugin (intentionally removed during monorepo refactor)
- `rag-v1/` — TypeScript RAG plugin (intentionally removed during monorepo refactor)
- `archive/legacy/` — retired code (intentionally removed during monorepo refactor)

### 8.2 Active Rust Surface

crabjar (binary) + crabjar-config (library) + agent-context (library)

archive/ excluded from build.

### 8.3 Build Commands

- just check: cargo check --workspace
- just build: cargo build -p crabjar
- just test: cargo test --workspace
- just clean: remove build artifacts

---

## 9. Drift Report

### Last Audit

2026-05-03 — workspace consolidation updated. `a-hole` directory removed from worktree. `mirror-guard` status updated from NEW to Core. Workspace members and deps aligned to root Cargo.toml.

### Known Items

- `mirror.db` files at root, `mirror-kernel/`, `mirror-log/` — runtime SQLite databases, `.gitignore`d
- Single Git repo — all nested `.git/` removed (2026-04-26). Each crate independently buildable, shareable, forkable
- Single `Cargo.lock` at workspace root — crate-level locks removed (2026-04-27)
- Single `LICENSE` (MIT OR Apache-2.0) at workspace root — crate-level licenses removed (2026-04-27)
- `crabjar/reference_materials/` — excluded from Git (cloned reference repos, not authored code)
- `zllg/` — confirmed workspace member, documented as multiplexing TUI IDE framework
- `a-hole/`, `js-code-sandbox/`, `rag-v1/`, `archive/legacy/` — intentionally removed from worktree
- `pipelines/` — empty, reserved for future pipeline definitions
- `mirror-ledger/` — non-crate artifact store: artifacts/, reflections/, work/, ledger.jsonl

### Provenance Entries

| UUID | Item | Set At | Reason | Source |
|---|---|---|---|---|
| `prov-agents-2026-05-03` | Provenance Boundaries added to AGENTS.md | 2026-05-03 | Phase 1 architectural formalization | AGENTS.md |
| `prov-map-drift-2026-05-03` | Drift section updated with provenance entries | 2026-05-03 | Phase 1 project_map alignment | crabjar/project_map.md |

---

*End of review.*
