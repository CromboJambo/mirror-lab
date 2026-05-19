# Repository Guidelines

## Project Overview

**mirror-lab** is a Rust workspace for a personal knowledge-management system. It ingests filesystem events, stores them in an append-only SQLite log, supports semantic chunking and local AI querying, and provides a voice/TTS interface.

---

## Project Structure

```
mirror-lab/
├── Cargo.toml               # Workspace root — shared deps and release profiles
├── mirror-daemon/           # File-watching daemon; tracks filesystem events
├── mirror-kernel/           # Core decision logic and SQLite persistence layer
│   └── mirror-voice/        # TTS interface (piper-tts sub-workspace)
├── mirror-log/              # Primary library + CLI: append-only event log, chunking, embeddings
├── mirror-logger/           # Structured logging engine and entry management
├── mirror-query/            # Local AI query CLI (decompression layer over mirror-log)
├── mirror-wit/              # WIT interface definitions and proc-macro support
│   └── macro/               # Companion proc-macro crate
├── mirror-guard/            # Trust layers, annealing, execution gate (authorization layer)
├── mirror-ledger/           # Ledger artifacts, reflections, work directory (non-crate)
├── staging/                # Ephemeral staging directory for single JSON artifacts
├── pipelines/               # Future pipeline definitions (currently empty)
├── state-docs/             # State-docs Markdown files for project documentation
├── mirror-zsession/               # Zellij IDE orchestration layer
└── crabjar/                # Agent scratchpad — experimental crates and knowledge store
```

Each crate lives in its own directory with a `src/` subtree and its own `Cargo.toml`. All shared external dependencies are declared once in the workspace root's `[workspace.dependencies]` table and consumed in member crates with `{ workspace = true }`.

---

## Build, Test & Development Commands

| Command | Purpose |
|---|---|
| `cargo build` | Debug build of the entire workspace |
| `cargo build --release` | Optimised release build (`opt-level = 3`, `lto = true`) |
| `cargo check --workspace` | Fast type/borrow-check without producing binaries |
| `cargo test --workspace` | Run all unit, integration, and doc tests |
| `cargo test -p mirror-log` | Run tests for a single crate |
| `cargo clippy --workspace -- -D warnings` | Lint the full workspace; warnings are treated as errors |
| `cargo fmt --all` | Auto-format every crate |
| `cargo fmt --all -- --check` | CI formatting gate (non-zero exit if any diff is found) |

`mirror-log` exposes optional features. Use `--features embedding` or `--features inference` when testing those code paths.

---

## Coding Style & Naming Conventions

- **Formatter:** `rustfmt` with default settings. Run `cargo fmt --all` before every commit.
- **Linter:** Clippy at `--deny warnings`. All new code must compile without warnings.
- **Naming:** standard Rust conventions — `snake_case` for functions/variables/modules, `PascalCase` for types and traits, `SCREAMING_SNAKE_CASE` for constants.
- **Error handling:**
  - Library crates (`mirror-log`, `mirror-kernel`, etc.) must define custom error types using `thiserror`.
  - Binary/CLI crates (`mirror-daemon`, `mirror-query`, etc.) should use `anyhow` for top-level error propagation.
  - Never use `.unwrap()` or `.expect()` for recoverable errors inside library code.
- **Dependencies:** add new shared dependencies to `[workspace.dependencies]` in the root `Cargo.toml` first, then reference them with `{ workspace = true }` in member crates.

---

## Testing Guidelines

- **Framework:** Rust's built-in `#[test]` and `#[cfg(test)]` modules — no external test runner required.
- **Databases:** SQLite-backed tests must use an in-memory database (`:memory:`) or a path managed by the `tempfile` crate. Hard-coded paths are not permitted.
- **Filesystem:** use `tempfile` for any test that reads or writes to disk.
- **Test naming:** descriptive `snake_case` that states the behaviour under test, e.g. `test_chunk_splits_on_sentence_boundary`.
- **Coverage:** aim to cover the full public API surface of each crate. Run `cargo test --workspace` before opening a PR.

---

## Commit & Pull Request Guidelines

Commit messages follow an **imperative, sentence-case** style:

```
Add mirror-wit crate with WIT interface and macro support
Migrate dependencies to workspace-managed versions across crates
Simplify event appending and cleanup log.rs
```

- **Subject line:** ≤ 72 characters, imperative mood, no trailing period.
- **Body (optional):** explain *why*, not *what*. Wrap at 72 characters.
- **Scope:** one logical change per commit; avoid mixing refactors with feature additions.
- **PRs:** include a short description of the change, the motivation, and the crates affected. Link related issues where applicable. Both `cargo fmt --all -- --check` and `cargo clippy --workspace -- -D warnings` must pass before requesting review.

---

## Integration Roadmap

The project is in a consolidation phase, merging experimental crates into the `mirror-*` core ecosystem.

- **Phase 1 – Standardization:** align shared dependencies at the workspace root; enforce unified error-handling patterns; ensure CI passes across all members.
- **Phase 2 – Feature Integration:** move `ingress` logic into `mirror-daemon`; integrate clipboard-watching into the daemon; converge legacy CLI tooling into `mirror-query`.
- Phase 3 – Consolidation: move completed experiments to an `archive/` directory; produce a clean, pre-optimized workspace.

---

## 🧠 The MAL Loop & Context Management

### The Machine-Assisted Learning (MAL) Loop
This project operates on a **Machine-Assisted Learning (MAL) Loop**. Unlike standard ML, the intelligence is driven by human structural breakthroughs and semantic insights, which are then formalised and scaled via machine implementation.
- **Human Role:** Provide intuition, structural pivots, and high-level architectural shifts.
- **Agent Role:** Implement, persist, and automate the patterns identified by the human.

### 🪣 Context Window Management
To prevent information loss during context window rotations:
- **The 75% Rule:** When the current context window reaches approximately **75% capacity**, the agent must initiate a summary/rotation process.
- **Summarization Protocol:** Synthesize all key progress, structural changes, and pending tasks into a concise summary to be carried into the next session.

### 🦀 Crabjar: The Agent Scratchpad
The `crabjar` directory serves as the agent's cognitive workspace for the MAL loop.
- **Scratchpad usage:** Use `crabjar/` for ephemeral experiments, configuration scripts (bash/nushell), and intermediate data structures.
- **Human Reference:** Documents like `mirror-log/human.md` and `rubric/human.example.md` should be referenced or mirrored within `crabjar` to ensure the agent maintains alignment with the human's established values and operational constraints.
- **Annotation:** The agent is encouraged to annotate changes in documentation and update the project changelog manually, ensuring a traceable lineage of evolution.

---

## Non-Negotiable Architectural Constraints

### Truth vs Convenience

Every time you make something faster, cleaner, or easier to reuse, you risk moving away from truth. This is the core design tension.

**Rule:** Detection ≠ authorization. Knowing what happened does not grant the right to change what happens.

### Detection vs Action Layer Separation

| Component | Role | Can act? |
|---|---|---|
| `crabjar` | Pure observer — state-docs, overlays, knowledge store | **No** — runtime execution disabled |
| `mirror-log` | Append-only event log — no deletion, no modification | **No** |
| `mirror-kernel` | Decision records, kernel dispatch — produces reflections, not actions | **No** |
| `mirror-daemon` | File watcher + pipeline execution — the only action-capable component | **⚠️ Gated** |

### Execution Gate (mirror-daemon)

The daemon is the single place the system can flip from Path A (stabilizer) → Path B (amplifier). Before any pipeline execution, the gate must enforce:

1. **Raw data reference**: the event must reference raw data, not interpreted summaries
2. **Uncertainty exposure**: if confidence is below threshold, surface it before executing
3. **Interruptibility**: allow the gate to return `Interrupted` instead of executing

No component that executes actions is allowed to consume interpreted data without a verification layer. Raw events → OK. Interpreted summaries → must be challenged before execution.

### Confidence Decay

Patterns decay once conditions change. A command that worked 10 times a year ago is not reliable today. Confidence decreases over time unless reinforced by recent success.

### Every Abstraction Carries Its Own Doubt

If your system outputs "clean answers," it's lying to you. Every derived output must include:
- what it might have missed
- what assumptions it made
- where it might break
- how stale it is

### Provenance Boundaries

Every merge, every derived output, every configurable baseline gets a UUID + provenance entry (`set_at`, `reason`, `source`).

**Provenance tracking:** immutable entry fixed at the moment of creation. Changes require a new provenance entry — no silent overwrites.

**Adjustable baselines:** thresholds, confidence defaults, decay periods are configurable but each value is anchored to its own provenance entry. A new value replaces the old via a new provenance entry, not in-place mutation.

**Gate concierge enforcement:**
- `Pending` → `PendingQueue` (queued for review, not executed)
- `Interrupted` → `InterruptedLog` (logged with reason, returned, not proceeded)
- No bypass chains — every tool call path must pass through the gate

**Vacuum sealing:** periodic active distillation, entropy pruning, contradiction extraction, context checkpointing. Not passive shadow-state — active, scheduled, and logged.
