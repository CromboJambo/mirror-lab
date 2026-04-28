# Repository Guidelines

## Project Structure & Module Organization

```
crabjar/
├── src/
│   ├── main.rs               # CLI entry point — command parsing and dispatch
│   ├── project_loader.rs     # Loads .crabjar_config.toml from the working directory
│   ├── state_docs.rs         # state-docs read/write and overlay annotation logic
│   ├── knowledge_store/      # Knowledge bridge (mod.rs, commands.rs)
│   └── crabjar-config/       # Workspace crate: config struct, TOML parsing
├── memory/files/             # agent-context crate: knowledge store (SQLite-backed)
├── tests/
│   └── cli.rs                # Integration tests that exercise the compiled binary
├── Cargo.toml                # Workspace root + crabjar binary manifest
└── Justfile                  # Task runner shortcuts
```

The active Rust surface is **`crabjar` (binary) + `crabjar-config` (library) + `agent-context` (library)**.

---

## Build, Test, and Development Commands

All common tasks are available via [`just`](https://github.com/casey/just). Run `just` with no arguments to list them.

| Command | What it does |
|---|---|
| `just check` | `cargo check --workspace` — fast type/borrow check, no artifacts |
| `just build` | `cargo build -p crabjar` — compile the CLI binary |
| `just run state list` | Run the binary with arbitrary args (default: `state list`) |
| `just test` | `cargo test --workspace` — unit + integration tests |
| `just clean` | Remove all build artifacts |

Raw Cargo equivalents work too (e.g., `cargo test -p crabjar`).

---

## Coding Style & Naming Conventions

- **Formatter**: `rustfmt` with default settings. Run `cargo fmt` before committing; CI treats formatting failures as errors (`cargo fmt --check`).
- **Linter**: `cargo clippy -- -D warnings`. Fix all warnings before opening a PR.
- **Naming**: follow standard Rust conventions — `snake_case` for functions/variables/modules, `PascalCase` for types/traits/enums, `SCREAMING_SNAKE_CASE` for constants.
- **Error handling**: use `thiserror` for library errors; propagate with `?`. Avoid `unwrap()` outside of tests.
- **Edition**: Rust 2024 (`edition = "2024"`). Use idiomatic edition-2024 patterns.
- **JSON output**: all CLI commands write structured JSON to stdout. Maintain this contract — do not add plain-text output paths.

---

## Testing Guidelines

- **Framework**: standard `#[test]` / `#[tokio::test]`. No external test framework.
- **Integration tests** live in `tests/cli.rs` and run the compiled binary via `std::process::Command`. Use `tempfile::tempdir()` for all filesystem fixtures — never write to the project directory.
- **Unit tests** belong in `#[cfg(test)]` modules inside the relevant source file.
- **Naming**: test function names should read as plain sentences describing the expected behaviour, e.g. `state_list_returns_json`, `missing_command_exits_nonzero`.
- **Coverage**: every new CLI subcommand or state-docs operation must have at least one integration test covering the happy path and one covering the error path.
- Run the full suite with `just test` before pushing.

---

## Commit & Pull Request Guidelines

**Commit messages** in this repository follow an imperative, descriptive style:

- Start with a capital verb: `Add`, `Fix`, `Remove`, `Refactor`, `Update`.
- Keep the subject line under ~72 characters.
- Use the body for context when the change is non-trivial (see the multi-bullet commit in the log for a good example).

```
Add overlay persistence for state-doc annotations

Writes resolved annotations back to state-docs/overlay/<doc>.overlay.json
so that agent sessions survive process restarts.
```

**Pull requests** should:

1. Reference the issue or context driving the change.
2. Include a brief description of *what* changed and *why*.
3. Pass `just check`, `just test`, `cargo fmt --check`, and `cargo clippy -- -D warnings` locally before requesting review.
4. Keep changes focused — avoid mixing refactors with feature additions in a single PR.

---

## Architecture Notes

- The CLI is **synchronous at the command-parsing layer** and async only where I/O requires it (Tokio runtime in `main`).
- State docs are Markdown files under `<project-root>/state-docs/`. Overlay annotations are stored as JSON sidecars in `state-docs/overlay/`.
- Workspace config is loaded from `.crabjar_config.toml` in the current working directory. A missing or malformed config is a soft failure — the CLI continues with `workspace: null`.

---

## Non-Negotiable Architectural Constraints

### Truth vs Convenience

Every time you make something faster, cleaner, or easier to reuse, you risk moving away from truth. This is the core design tension.

**Rule:** Detection ≠ authorization. Knowing what happened does not grant the right to change what happens.

### Detection vs Action Layer Separation

| Component | Role | Can act? |
|---|---|---|
| `crabjar` | Execution engine — state-docs, overlays, knowledge store, tool execution | **⚠️ Gated** |
| `mirror-log` | Append-only event log — no deletion, no modification | **No** |
| `mirror-kernel` | Decision records, kernel dispatch — produces reflections, not actions | **No** |
| `mirror-daemon` | File watcher + pipeline execution — the only action-capable component | **⚠️ Gated** |

### Execution Gate (crabjar + mirror-daemon)

Before any tool execution, the gate must enforce:

1. **Raw data reference**: the event must reference raw data, not interpreted summaries
2. **Uncertainty exposure**: if confidence is below threshold, surface it before executing
3. **Interruptibility**: allow the gate to return `Interrupted` instead of executing
4. **Reversibility scoring**: scan tool calls for reversibility; request permission if reversibility or other risk factors exceed established threshold

No component that executes actions is allowed to consume interpreted data without a verification layer. Raw events → OK. Interpreted summaries → must be challenged before execution.

### Confidence Decay

Patterns decay once conditions change. A command that worked 10 times a year ago is not reliable today. Confidence decreases over time unless reinforced by recent success.

### Every Abstraction Carries Its Own Doubt

If your system outputs "clean answers," it's lying to you. Every derived output must include:
- what it might have missed
- what assumptions it made
- where it might break
- how stale it is

### Context Preservation

Responses must stay under 4 lines of text (not including tool use). Massive responses consume conversation context — when the user asks to continue, the agent has no context to continue with. Keep responses short.
