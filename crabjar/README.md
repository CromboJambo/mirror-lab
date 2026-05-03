# Crabjar

Crabjar is currently a stripped-down Rust CLI for local `state-docs` management. The supported product surface is the root `crabjar` binary plus the local workspace/config loader and `state-docs` overlay support.

## Supported Path

Active components:

- `src/main.rs`: the only supported CLI entrypoint.
- `src/project_loader.rs`: workspace command/config loader used by the root binary.
- `src/state_docs.rs`: local state-docs manager with overlay annotations.
- `src/crabjar-config/`: TOML config crate used by the root binary.

Legacy code has been consolidated into the `mirror-*` core ecosystem or removed from the worktree.

## Commands

```bash
just check
just build
just run state list
just test
```

Without `just`:

```bash
cargo check --workspace
cargo build -p crabjar
cargo run -p crabjar -- state list
cargo test --workspace
```

## Current State

- `cargo check --workspace` should pass.
- `cargo build -p crabjar` should pass.
- `cargo test --workspace` should pass.
- `cargo run -p crabjar -- --help` returns structured JSON.
- Runtime tool execution is intentionally disabled in this stripped-down build.
- `state-docs/` is the supported feature surface for local project memory and annotations.
- All supported CLI responses are structured JSON on stdout, including help and error paths.

## Architectural Constraints

### Detection ≠ Authorization

Crabjar is a pure observer. It knows what happened but cannot change what happens. This is enforced by design — no pipeline execution, no file modification beyond append-only overlays.

### Doubt Output Requirement

Every derived output must include a `doubt` block with:
- `assumptions` — what it assumed to produce this output
- `blind_spots` — what it couldn't see
- `last_validation` — when this was last checked against raw data
- `stale_after` — when this output should be considered stale

If a response lacks this block, it is not allowed to exist.

## State Docs

Crabjar can treat `state-docs/` as a shared project memory surface. The Markdown files remain the durable source documents, and agent/user comments live in `state-docs/overlay/*.overlay.json` so they can be updated without rewriting the base docs.

Available commands:

```bash
crabjar state list
crabjar state show crabjar_state
crabjar state annotate crabjar_state "Needs a tighter ABI milestone summary"
crabjar state question crabjar_state "Should this move under docs/ instead?"
crabjar state resolve crabjar_state <annotation-id>
crabjar workspace status
crabjar knowledge sync crabjar_state
crabjar knowledge query --tags=state-doc
crabjar knowledge events --limit=20
crabjar knowledge verify
crabjar knowledge deactivate <id> --reason="superseded"
```

## Output Contract

- All command responses are JSON written to stdout.
- Successful responses include `"success": true`.
- Error responses include `"success": false`, an `"error"` string, and usually a `"usage"` array.
- `workspace status` returns `"workspace": null` when `.crabjar_config.toml` is missing or malformed.
- `knowledge` subcommands return structured fields such as `rows`, `events`, `docs`, `ids`, or `id` instead of plain-text summaries.

Example help response:

```json
{
  "success": true,
  "error": null,
  "usage": [
    "crabjar state list",
    "crabjar state show <doc>",
    "crabjar state annotate <doc> <message>",
    "crabjar state question <doc> <message>",
    "crabjar state resolve <doc> <id>",
    "crabjar workspace status",
    "crabjar knowledge <subcommand>"
  ]
}
```

## Near-Term Goal

Keep the reduced path solid before reintroducing any executor/runtime work:

1. Keep one binary and one truthful README.
2. Add end-to-end tests around CLI behavior and `state-docs`.
3. Decide whether runtime execution returns as native Rust, WASM, or not at all.
4. Keep legacy implementations quarantined under `archive/` until they are either restored intentionally or deleted.
5. Add doubt block to all derived JSON outputs (assumptions, blind_spots, last_validation, stale_after).
