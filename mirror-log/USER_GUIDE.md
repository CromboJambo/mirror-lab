# Mirror-Log User Guide

`mirror-log` 0.1.9 combines a persisted SQLite event log with a file-based staging area for review-oriented workflows. This guide describes the current behavior of the shipped CLI, not the earlier roadmap or deleted pipeline docs.

## Concepts

There are two kinds of data in the current system.

- Persisted events live in `mirror.db` and are the append-only log queried by `show`, `search`, `stats`, `info`, and `verify`
- Staged events live as JSON files in `staging/` and are used by `review`, `infer`, and `regenerate`

The important operational detail is that not every ingest command behaves the same way.

- `add` stages a single event and does not persist it to SQLite
- `add-file` stages a file as a single event and does not persist it to SQLite
- `stdin` persists batched events to SQLite and then writes staged copies for review

## Installation

```bash
git clone https://github.com/CromboJambo/mirror-log
cd mirror-log
cargo build --release
```

The binary will be at `target/release/mirror-log`.

To install locally:

```bash
cargo install --path .
```

Optional builds:

```bash
cargo build --release --features embedding
cargo build --release --features inference
cargo build --release --features iteration
cargo build --release --features clipboard
```

## Core Commands

### Stage Single Events

```bash
mirror-log add "Overhead allocation needs review" --source journal
mirror-log add "Meeting notes" --source meetings --meta '{"important":true}'
mirror-log add-file notes.md --source meetings
```

These commands create `StagedEvent` JSON files in `staging/`.

### Batch Ingest From Stdin

```bash
cat ideas.txt | mirror-log stdin --source ideas
printf "Event 1\nEvent 2\nEvent 3\n" | mirror-log stdin --source inbox
```

`stdin` uses the SQLite ingest pipeline and chunking policy, then emits staged copies of the persisted events.

### Review Staged Data

```bash
mirror-log review
mirror-log infer
mirror-log regenerate --output human.md
```

- `review` lists staged events
- `infer` runs simple pattern detection across staged events
- `regenerate` renders staged events back out; the default output argument is `human.md`

`regenerate` currently prints rendered content to stdout. It does not rewrite files on disk.

### Query Persisted Events

```bash
mirror-log show --last 10
mirror-log show --source journal --last 5
mirror-log search "allocation"
mirror-log search "allocation" --chunks
mirror-log get <event-id>
mirror-log stats
mirror-log info
mirror-log verify
```

- `show` lists recent persisted events
- `search` matches full event content
- `search --chunks` queries the chunk table
- `verify` checks stored hashes and relational integrity

## Attention Layer

The attention layer is part of the default build in 0.1.9.

```bash
mirror-log attention
mirror-log attention --flagged
mirror-log attention --stats
mirror-log add-to-attention <event-id>
```

It operates on persisted events and uses the `decay` and `shadow_state` tables to surface active items, flagged items, and counts.

## Chunking

Large persisted events are chunked automatically during pipeline ingestion.

- Auto-chunk threshold: `2000` bytes
- Default chunk size: `1500` bytes
- Chunk boundaries prefer whitespace when possible

Chunk search is available through `mirror-log search <term> --chunks`.

## Storage Layout

Default paths:

- SQLite database: `mirror.db`
- Staging directory: `staging/`

Important persisted tables:

- `events`
- `chunks`
- `decay`
- `shadow_state`
- `event_tags`
- `event_links`
- `event_embeddings`
- iteration tables in `src/schema.sql`

Staged files are JSON representations of `StagedEvent` values and are separate from the append-only log.

## Features

The current Cargo features are:

- `embedding`
- `inference`
- `iteration`
- `clipboard`

Behavior notes:

- attention, decay, staging, and pattern inference are in the default build
- embedding commands are only available with `--features embedding`
- clipboard support is only compiled with `--features clipboard`

## Direct SQLite Queries

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

## Troubleshooting

If `review`, `infer`, or `regenerate` show nothing:

- confirm `staging/` exists
- confirm you used `add` or `add-file`, or that `stdin` completed successfully

If `show` or `search` show nothing:

- confirm you persisted events through `stdin` or library APIs
- check that events were not moved into `shadow_state`

If embedding commands are missing:

- rebuild with `cargo build --release --features embedding`
