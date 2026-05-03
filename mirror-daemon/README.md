# Mirror Daemon

A witness daemon for deterministic data pipelines.

## Philosophy

Mirror is **not**:
- An AI service
- A background script runner
- A "helpful" assistant
- A database

Mirror **is**:
- A witness that observes pipeline executions
- A ledger that remembers what happened
- A seal that guarantees integrity
- A boring, strict, lawful process

## Core Principles

1. **Append-only**: Nothing is ever overwritten
2. **Witness, don't decide**: The daemon observes and seals, it doesn't interpret
3. **Artifacts over state**: Everything is externalized, nothing hidden
4. **Boring over clever**: Simple, deterministic, auditable
5. **Detection ≠ Authorization**: Knowing what happened does not grant the right to change what happens
6. **Truth vs Convenience**: Every time you make something faster, cleaner, or easier to reuse, you risk moving away from truth

## Architecture

```
Pipeline (.nu script)
    ↓
Executor (runs in isolation)
    ↓
Reflection Envelope (sealed artifact)
    ↓
Ledger (append-only journal)
```

## Directory Structure

```
mirror-ledger/
├── ledger.jsonl              # Append-only journal
├── reflections/              # Content-addressed envelopes
│   └── ab/
│       └── abc123.../
│           └── meta.json
├── artifacts/                # Content-addressed outputs
│   └── de/
│       └── def456...
└── work/                     # Temporary execution spaces
```

## Usage

### Run a pipeline
```bash
mirror run my-pipeline.nu
```

This:
1. Executes the pipeline in isolation
2. Captures all outputs
3. Hashes everything
4. Seals a reflection envelope
5. Appends to the ledger

### List available pipelines
```bash
mirror list
```

### View recent executions
```bash
mirror recent --limit 20
```

### Inspect a specific reflection
```bash
mirror inspect abc123...
```

### View ledger statistics
```bash
mirror stats
```

## Pipeline Format

Pipelines are Nu scripts stored in the `pipelines/` directory (currently empty — reserved for future definitions).

Example `pipelines/cashflow.nu`:
```nu
# Generate a simple cashflow report
let data = [
    [month revenue expenses];
    [Jan 10000 8000]
    [Feb 12000 9000]
    [Mar 11000 8500]
]

$data | to json | save cashflow.json
```

When executed, the daemon:
- Hashes the script content
- Runs it in an isolated directory
- Captures stdout/stderr
- Collects any generated files
- Seals everything in a reflection envelope

## What Makes This Different

Traditional approaches:
- **Databases**: Hide data behind queries, mutable state
- **CI/CD**: Focus on deployment, not data lineage
- **Workflow engines**: Complex orchestration, hard to audit
- **LLMs**: Probabilistic, opaque, unstable

Mirror:
- **Everything is an artifact**: No hidden state
- **Time is sacred**: Append-only ledger preserves all history
- **Deterministic**: Same input + same pipeline = same hash
- **Inspectable**: Every reflection can be examined
- **Boring**: Simple file system layout, JSON metadata

## Execution Gate (non-negotiable)

The daemon is the single place the system can flip from Path A (stabilizer) → Path B (amplifier). Before any pipeline execution, `mirror-guard` enforces:

1. **Raw data reference**: the event must reference raw data, not interpreted summaries
2. **Uncertainty exposure**: if confidence is below threshold, surface it before executing
3. **Interruptibility**: allow the gate to return `Interrupted` instead of executing
4. **Trust layer auto-execute check**: layer 3 `annealed` (0.8–1.0) allows auto-execute
5. **Command risk assessment**: high-risk → Interrupted; medium-risk → Pending; low-risk → Proceed

No component that executes actions is allowed to consume interpreted data without a verification layer. Raw events → OK. Interpreted summaries → must be challenged before execution.

## Future Directions

Potential extensions (not implemented yet):
- Input fingerprinting (track data sources)
- Declarative scheduling (causality-based, not time-based)
- Diff operations (compare reflections)
- Git integration (track pipeline versions)
- Cryptographic signatures (witness authentication)

## Building

```bash
cargo build --release
```

## Testing

```bash
cargo test
```

## Installation

```bash
cargo install --path .
```

## License

This is experimental software. Use at your own risk.

## Philosophy Attribution

This daemon embodies the ideas from the "Large Data Model" concept:
- Deterministic over probabilistic
- Artifacts over latent states
- Witness over authority
- Durable over trendy

The goal is not to be clever. The goal is to outlive everything flashier.
