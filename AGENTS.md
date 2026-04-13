# Repository Guidelines (and Agent Instructions)

## Project Overview

**mirror-lab** is a Rust workspace for a personal knowledge-management system. It ingests events, stores them in an append-only SQLite log, supports semantic chunking and local AI querying, and provides a voice/TTS interface.

## Integration Roadmap

The project is currently in a major consolidation phase, merging several experimental and auxiliary crates (e.g., `crab-cli`, `crab_tui`, `ingress`) into the unified `mirror-*` core ecosystem.

### Phase 1: Standardization
- **Dependency Alignment**: Migrating common dependencies (e.g., `tokio`, `thiserror`) to the workspace root.
- **Unified Error Handling**: Enforcing `thiserror` for libraries and `anyhow` for binaries.
- **CI/CD Readiness**: Ensuring all members pass unified linting and formatting checks.

### Phase 2: Feature Integration
- **Ingress Expansion**: Moving `ingress` logic into `mirror-daemon`.
- **Event Source Expansion**: Integrating `clipboard-tts` as a watcher in `mirror-daemon`.
- **UI/CLI Convergence**: Merging `crab-cli` and `crab_tui` capabilities into `mirror-query` and new high-level interfaces.

### Phase 3: Consolidation
- **Archive Cleanup**: Moving completed experiments to an `archive/` directory.
- **Final Workspace Polish**: A unified, single-purpose workspace structure.

---

## Project Structure

```
mirror-lab/
├── Cargo.toml          # Workspace root — shared deps and profiles
├── mirror-daemon/      # File-watching daemon; tracks filesystem events
├── mirror-kernel/      # Core decision logic and SQLite persistence layer
│   └── mirror-log/      # (Relocated) Primary library + CLI: append-only event log, chunking, embeddings
├── mirror-logger/      # Structured logging engine and entry management
├── mirror-query/       # Local AI query CLI (decompression layer over mirror-log)
├── mirror-voice/       # TTS interface (piper-tts sub-workspace)
└── mirror-wit/         # WIT interface definitions and proc-macro support
```

For specialized agent instructions, configuration, and the "Dreaming Mode" protocol, refer to: [crabjar/agent_config.md](./crabjar/agent_config.md)

Each crate lives in its own directory with a `src/` subtree and its own `Cargo.toml`. Shared dependencies are declared once in the workspace root's `[workspace.dependencies]` table.

---

## Build, Test & Development Commands

| Command | Purpose |
|---|---|
| `cargo build` | Debug build of the entire workspace |
| `cargo build --release` | Optimised release build (`opt-level = 3`, `lto = true`) |
| `cargo check --workspace` | Fast type/borrow-check without producing binaries |
| cargo test --workspace | Run all unit, integration, and doc tests for the entire workspace. |
| `cargo test -p mirror-log` | Run tests for a single crate |
| `cargo clippy --workspace -- -D warnings` | Lint the full workspace; warnings are errors |
| `cargo fmt --all` | Auto-format every crate with `rustfmt` |
| `cargo fmt --all -- --check` | CI formatting gate (non-zero exit if diff found) |

> **Tip:** `mirror-log` exposes optional features. Use `--features embedding` or `--features inference` when testing those code paths.

---

## Coding Style & Naming Conventions

- **Formatter:** `rustfmt` with default settings. Run `cargo fmt --all` before committing.
- **Linter:** Clippy at `--deny warnings`. New code must compile without warnings.
- **Naming:** follow standard Rust conventions — `snake_case` for functions/variables/modules, `PascalCase` for types and traits, `SCREAMING_SNAKE_CASE` for constants.
- **Error handling:** Library crates must define custom errors using `thiserror`. Binary/CLI applications should handle top-level error propagation using `anyhow`. Never use `.unwrap()` or `.expect()` for recoverable errors within library code.
- **Dependencies:** All commonly used external dependencies must be declared once in the workspace root's `[workspace.dependencies]`. Member crates must then consume these dependencies using `dep = { workspace = true }`.

---

## Testing Guidelines

- **Framework:** Rust's built-in `#[test]` and `#[cfg(test)]` modules.
- **Filesystem/Resources:** When testing file system interactions, always use the `tempfile` crate. SQLite tests must use either an in-memory database (`:memory:`) or a path managed by `tempfile` — never hard-coded paths.
- **Test names:** use descriptive `snake_case` names that state the behaviour under test, e.g. `test_chunk_splits_on_sentence_boundary`.
- **Coverage target:** aim to test all public API surface. SQLite-backed tests must use an in-memory DB (`:memory:`) or a `tempfile`-managed path — never a hard-coded path.
- Run the full suite before opening a PR: `cargo test --workspace`.

---

## Commit & Pull Request Guidelines

Commit messages follow an **imperative, sentence-case** style observed in the project history:

```
Add mirror-wit crate with WIT interface and macro support
Migrate dependencies to workspace-managed versions across crates
Convert project from Python to Rust workspace
```

- **Subject line:** ≤ 72 characters, imperative mood, no trailing period.
- **Body (optional):** explain *why*, not *what*. Wrap at 72 characters.
- **Scope:** prefer one logical change per commit; avoid mixing refactors with feature additions.
- **PRs:** include a short description of the change, the motivation, and any crates affected. Link related issues where applicable. Ensure `cargo fmt --all -- --check` and `cargo clippy --workspace -- -D warnings` both pass before requesting review.
