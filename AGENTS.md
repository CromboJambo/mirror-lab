# Repository Guidelines

## Project Overview

**mirror-lab** is a Rust workspace for a personal knowledge-management system. It ingests events, stores them in an append-only SQLite log, supports semantic chunking and local AI querying, and provides a voice/TTS interface.

---

## Project Structure

```
mirror-lab/
├── Cargo.toml          # Workspace root — shared deps and profiles
├── mirror-daemon/      # File-watching daemon; tracks filesystem events
├── mirror-kernel/      # Core decision logic and SQLite persistence layer
├── mirror-log/         # Primary library + CLI: append-only event log, chunking, embeddings
├── mirror-logger/      # Structured logging engine and entry management
├── mirror-query/       # Local AI query CLI (decompression layer over mirror-log)
├── mirror-voice/       # TTS interface (piper-tts sub-workspace)
└── mirror-wit/         # WIT interface definitions and proc-macro support
```

Each crate lives in its own directory with a `src/` subtree and its own `Cargo.toml`. Shared dependencies are declared once in the workspace root's `[workspace.dependencies]` table.

---

## Build, Test & Development Commands

| Command | Purpose |
|---|---|
| `cargo build` | Debug build of the entire workspace |
| `cargo build --release` | Optimised release build (`opt-level = 3`, `lto = true`) |
| `cargo check --workspace` | Fast type/borrow-check without producing binaries |
| `cargo test --workspace` | Run all unit, integration, and doc tests |
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
- **Error handling:** use `thiserror` for library errors (`mirror-log`); use `anyhow` for binary/CLI error propagation (`mirror-query`). Avoid `.unwrap()` in library code.
- **Dependencies:** always declare shared deps in `[workspace.dependencies]` and inherit them with `dep = { workspace = true }` in member crates.

---

## Testing Guidelines

- **Framework:** Rust's built-in `#[test]` and `#[cfg(test)]` modules.
- **Temporary files:** use the `tempfile` crate (already a dev-dependency in `mirror-daemon`) for any test that touches the filesystem.
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
