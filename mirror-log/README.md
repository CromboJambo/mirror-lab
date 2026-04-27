```
███▄ ▄███▓ ██▓ ██▀███   ██▀███   ▒█████   ██▀███   ██▓     ▒█████    ▄████ 
▓██▒▀█▀ ██▒▓██▒▓██ ▒ ██▒▓██ ▒ ██▒▒██▒  ██▒▓██ ▒ ██▒▓██▒    ▒██▒  ██▒ ██▒ ▀█▒
▓██    ▓██░▒██▒▓██ ░▄█ ▒▓██ ░▄█ ▒▒██░  ██▒▓██ ░▄█ ▒▒██░    ▒██░  ██▒▒██░▄▄▄░
▒██    ▒██ ░██░▒██▀▀█▄  ▒██▀▀█▄  ▒██   ██░▒██▀▀█▄  ▒██░    ▒██   ██░░▓█  ██▓
▒██▒   ░██▒░██░░██▓ ▒██▒░██▓ ▒██▒░ ████▓▒░░██▓ ▒██▒░██████▒░ ████▓▒░░▒▓███▀▒
░ ▒░   ░  ░░▓  ░ ▒▓ ░▒▓░░ ▒▓ ░▒▓░░ ▒░▒░▒░ ░ ▒▓ ░▒▓░░ ▒░▓  ░░ ▒░▒░▒░  ░▒   ▒ 
░  ░      ░ ▒ ░  ░▒ ░ ▒░  ░▒ ░ ▒░  ░ ▒ ▒░   ░▒ ░ ▒░░ ░ ▒  ░  ░ ▒ ▒░   ░   ░ 
░  ░      ░ ▒ ░  ░▒ ░ ▒░  ░▒ ░ ▒░  ░ ▒ ▒░   ░▒ ░ ▒░░ ░ ▒  ░  ░ ▒ ▒░   ░   ░ 
░ ▒░   ░  ░░▓  ░ ▒▓ ░▒▓░░ ▒▓ ░▒▓░░ ▒░▒░▒░ ░ ▒▓ ░▒▓░░ ▒░▓  ░░ ▒░▒░▒░  ░▒   ▒ 
▒██▒   ░██▒░██░░██▓ ▒██▒░██▓ ▒██▒░ ████▓▒░░██▓ ▒██▒░██████▒░ ████▓▒░░▒▓███▄▒
▒██    ▒██ ░██░▒██▄▄█▀  ▒██▄▄█▀  ▒██   ██░▒██▄▄█▀  ▒██░    ▒██   ██░░▓█  ██▓
▓██    ▓██░▒██▒▓██ ░▀█ ▒▓██ ░▀█ ▒▒██░  ██▒▓██ ░▀█ ▒▒██░    ▒██░  ██▒▒██░▀▀▀░
```
`mirror-log` is a local-first event log designed for capturing, structuring, and managing context. Beyond simple persistence in SQLite, it implements an active attention layer to manage the boundary between cold storage and active thought via decay, promotion, and structural salience.

[![CI](https://github.com/CromboJambo/mirror-log/actions/workflows/ci.yml/badge.svg)](https://github.com/CromboJambo/mirror-log/actions/workflows/ci.yml)
[![Release](https://github.com/CromboJambo/mirror-log/actions/workflows/release.yml/badge.svg)](https://github.com/CromboJambo/mirror-log/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/mirror-log.svg)](https://crates.io/crates/mirror-log)
[![Docs.rs](https://docs.rs/mirror-log/badge.svg)](https://docs.rs/mirror-log)
[![License](https://img.shields.io/crates/l/mirror-log.svg)](https://github.com/CromboJambo/mirror-log/blob/main/LICENSE)

Version `0.1.9` reflects the current shipped behavior: a lean SQLite core, staging helpers for review-oriented workflows, an attention/decay layer, and optional embedding or inference integrations behind Cargo features.

## What It Does

- Persists events to SQLite with UUIDs, timestamps, hashes, and chunk records
- Stages reviewable JSON events in `staging/` for curation
- Implements an active attention layer (decay, promotion, and structural salience) to surface relevant context
- Searches full events and chunked content
- Tracks duplicates by `content_hash`
- Supports optional embeddings, inference backends, iteration tables, and clipboard ingestion

## Current Workflow

```bash
# Stage a single event as JSON in ./staging
mirror-log add "Overhead allocation needs review" --source journal

# Stage a file as JSON in ./staging
mirror-log add-file notes.md --source meetings

# Persist stdin events to SQLite in batches, then also write staged copies
cat ideas.txt | mirror-log stdin --source ideas

# Review staged events
mirror-log review

# Detect simple patterns from staged events
mirror-log infer

# Render staged events back out
mirror-log regenerate --output human.md

# Query persisted events
mirror-log show --last 10
mirror-log search "allocation"
mirror-log search "allocation" --chunks
mirror-log get <event-id>

# Database and integrity views
mirror-log stats
mirror-log info
mirror-log verify

# Attention layer
mirror-log attention
mirror-log attention --flagged
mirror-log attention --stats
mirror-log add-to-attention <event-id>
```

## Installation

```bash
git clone https://github.com/CromboJambo/mirror-log
cd mirror-log
cargo build --release

# Optional feature paths
cargo build --release --features embedding
cargo build --release --features inference
cargo build --release --features clipboard

# Or install locally
cargo install --path .
```

The binary is written to `target/release/mirror-log`.

## Storage Model

Persisted log data lives in `mirror.db` by default.

- `events` stores the canonical persisted event log
- `chunks` stores chunked slices for large content and chunk search
- `decay` and `shadow_state` support attention/visibility behavior
- `event_embeddings`, `event_tags`, `event_links`, and iteration tables support enrichment-oriented workflows

Staged data is separate from the persisted log.

- `add` and `add-file` write `StagedEvent` JSON files to `staging/`
- `stdin` currently persists events to SQLite first and then emits staged copies for review
- Persisted events are append-only; staged files are a working set

## Features

The default build is intentionally lean.

- `embedding`: tokenizers-backed embedding generation and similarity search
- `inference`: HTTP inference backend support
- `iteration`: iteration query and type exports
- `clipboard`: clipboard watcher support

Attention and staging are part of the current default build, not separate feature flags.

## Documentation

- [USER_GUIDE.md](USER_GUIDE.md): usage, storage, and workflow details
- [src/lib.rs](src/lib.rs): public module and re-export surface
- [CHANGELOG.md](CHANGELOG.md): release history
- For the underlying philosophical framework (the "Why"), refer to the `a-hole/` directory in the workspace root.

## Direct SQLite Access

```sql
SELECT datetime(timestamp, 'unixepoch'), source, content
FROM events
ORDER BY timestamp DESC
LIMIT 10;

SELECT source, COUNT(*)
FROM events
GROUP BY source
ORDER BY COUNT(*) DESC;

SELECT COUNT(*) AS total,
       COUNT(DISTINCT content_hash) AS unique_events
FROM events;
```

## Development

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Dependency audit helpers:

```bash
scripts/dep-audit.sh
scripts/dep-audit.sh --features embedding
scripts/log-dep-audit.sh
scripts/log-dep-audit-matrix.sh
```

## License

AGPL-3.0-or-later. See `LICENSE`.
