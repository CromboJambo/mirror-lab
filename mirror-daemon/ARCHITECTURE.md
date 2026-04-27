# Mirror Daemon Architecture

## Core Abstraction: The Reflection

A **reflection** is an immutable witness of a pipeline execution.

### Reflection Envelope Structure
```rust
ReflectionEnvelope {
    id: String,                    // Content hash
    timestamp: DateTime,           // When executed
    transform: TransformWitness,   // What ran
    inputs: Vec<InputFingerprint>, // What it read
    outputs: Vec<OutputArtifact>,  // What it produced
    execution: ExecutionMeta,      // How it went
}
```

## Component Architecture

```
┌─────────────────────────────────────────────┐
│              CLI Client                      │
│         (mirror command)                     │
└──────────────┬──────────────────────────────┘
               │
               ↓
┌─────────────────────────────────────────────┐
│          Mirror Daemon                       │
│                                              │
│  ┌─────────────┐      ┌──────────────┐     │
│  │   Ledger    │◄─────┤   Executor   │     │
│  │ (append-only)│      │  (isolate &  │     │
│  │              │      │   capture)   │     │
│  └─────────────┘      └──────────────┘     │
└──────────────┬──────────────────────────────┘
               │
               ↓
┌─────────────────────────────────────────────┐
│           File System                        │
│                                              │
│  ledger.jsonl   (append-only journal)       │
│  reflections/   (sealed envelopes)          │
│  artifacts/     (content-addressed storage) │
│  work/          (temporary execution)       │
└─────────────────────────────────────────────┘
```

## Execution Gate

Before any pipeline execution, the gate must enforce three conditions:

1. **Raw data reference**: the event must reference raw data, not interpreted summaries
2. **Uncertainty exposure**: if confidence is below threshold, surface it before executing
3. **Interruptibility**: allow the gate to return `Interrupted` instead of executing

No component that executes actions is allowed to consume interpreted data without a verification layer. Raw events → OK. Interpreted summaries → must be challenged before execution.

Updated architecture:

```
┌─────────────────────────────────────────────┐
│              CLI Client                      │
│         (mirror command)                     │
└──────────────┬──────────────────────────────┘
               │
               ↓
┌─────────────────────────────────────────────┐
│          Mirror Daemon                       │
│                                              │
│  ┌─────────────┐      ┌──────────────┐     │
│  │   Ledger    │◄─────┤   Executor   │     │
│  │ (append-only)│      │  (isolate &  │     │
│  │              │      │   capture)   │     │
│  └─────────────┘      └──────────────┘     │
│                                              │
│  ┌─────────────┐                             │
│  │  Execution  │◄─────┤   Gate             │     │
│  │   Gate      │      │  (raw ref,         │     │
│  │             │      │  uncertainty,      │     │
│  │             │      │  interrupt)        │     │
│  └─────────────┘                             │
└──────────────┬──────────────────────────────┘
               │
               ↓
┌─────────────────────────────────────────────┐
│           File System                        │
│                                              │
│  ledger.jsonl   (append-only journal)       │
│  reflections/   (sealed envelopes)          │
│  artifacts/     (content-addressed storage) │
│  work/          (temporary execution)       │
└─────────────────────────────────────────────┘
```

## Data Flow

### Execution Flow
```
1. User: mirror run pipeline.nu
   ↓
2. Daemon: Read pipeline source
   ↓
3. Daemon: Hash pipeline content
   ↓
4. Executor: Create isolated work directory
   ↓
5. Executor: Run `nu pipeline.nu` in isolation
   ↓
6. Executor: Capture stdout, stderr, exit code
   ↓
7. Executor: Scan for output files
   ↓
8. Executor: Hash all outputs
   ↓
9. Daemon: Build ReflectionEnvelope
   ↓
10. Ledger: Write envelope to reflections/
    ↓
11. Ledger: Append entry to ledger.jsonl
    ↓
12. Return: Reflection ID to user
```

### Read Flow
```
1. User: mirror inspect <id>
   ↓
2. Daemon: Look up in ledger
   ↓
3. Ledger: Read envelope from reflections/
   ↓
4. Return: Display reflection details
```

## Key Design Decisions

### 1. Append-Only Ledger

**Why**: Time is sacred. Nothing should be overwritten.

**How**: 
- `ledger.jsonl` is append-only
- Each line is one `LedgerEntry`
- No delete operations
- No update operations
- Only append

**Trade-off**: Disk space grows unbounded. This is intentional.

### 2. Content-Addressed Storage

**Why**: Deduplication and integrity.

**How**:
```
artifacts/
  ├── ab/
  │   └── abc123def456...  (SHA256 of content)
  └── cd/
      └── cde789ghi012...
```

If two pipelines produce identical output, they share the same artifact.

**Trade-off**: Cannot store metadata in filename. Must use separate metadata file.

### 3. Isolated Execution

**Why**: Reproducibility and safety.

**How**:
- Each execution gets a unique work directory
- Pipeline runs with that directory as CWD
- Only files in work directory are captured as outputs

**Trade-off**: Pipelines cannot access parent directories or absolute paths easily.

### 4. No Database

**Why**: Boring is durable. Files outlive software.

**How**:
- JSONL for ledger (line-oriented, appendable)
- JSON for metadata (human-readable, tool-friendly)
- Directory structure for organization

**Trade-off**: No SQL queries. Must use `jq` or similar tools.

### 5. Hash Everything

**Why**: Integrity and provenance.

**What gets hashed**:
- Pipeline source code
- Input data
- Output artifacts
- Reflection envelope itself

**Trade-off**: Hashing takes time. Worth it.

### 6. No "Smart" Features

**Deliberately excluded**:
- Automatic retries
- Error recovery
- Dependency resolution
- Parallelization
- Caching
- Optimization

**Why**: These add complexity and break auditability. If you need them, build them as **separate layers on top** of the daemon.

## Storage Layout

### Directory Structure
```
mirror-ledger/
├── ledger.jsonl              # One line per reflection
│                             # Format: LedgerEntry JSON
│
├── reflections/              # Sealed envelopes
│   ├── ab/                   # First 2 chars of ID (sharding)
│   │   └── abc123.../        # Full ID
│   │       └── meta.json     # ReflectionEnvelope
│   └── cd/
│       └── cde456.../
│           └── meta.json
│
├── artifacts/                # Content-addressed outputs
│   ├── de/                   # First 2 chars of hash
│   │   └── def789...         # Full hash = content
│   └── fg/
│       └── fgh012...
│
└── work/                     # Temporary execution spaces
    ├── exec_1234567890/      # Timestamp-based
    ├── exec_1234567891/
    └── ...
```

### Ledger Format (JSONL)

Each line in `ledger.jsonl`:
```json
{"reflection_id":"abc...","ledger_time":"2025-02-01T12:00:00Z","envelope_path":"reflections/ab/abc.../meta.json","pipeline":"cashflow.nu","success":true}
```

### Reflection Envelope Format (JSON)

`reflections/ab/abc123.../meta.json`:
```json
{
  "id": "abc123def456...",
  "timestamp": "2025-02-01T12:00:00Z",
  "transform": {
    "content_hash": "def456ghi789...",
    "source_path": "pipelines/cashflow.nu",
    "version": null
  },
  "inputs": [],
  "outputs": [
    {
      "path": "work/exec_123/cashflow.json",
      "hash": "ghi789jkl012...",
      "artifact_type": "json"
    }
  ],
  "execution": {
    "exit_code": 0,
    "stdout": "Cashflow Report Generated\n...",
    "stderr": "",
    "duration_ms": 45,
    "witness": "cli"
  }
}
```

## Guarantees

### What Mirror Guarantees

1. **Append-only**: Ledger entries are never deleted or modified
2. **Content integrity**: Hashes match content
3. **Temporal ordering**: Ledger preserves execution order
4. **Isolation**: Each execution is independent
5. **Capture completeness**: All stdout/stderr/files are captured

### What Mirror Does NOT Guarantee

1. **Pipeline correctness**: Garbage in → garbage out
2. **Input stability**: External data sources may change
3. **Determinism**: Nu scripts may be non-deterministic
4. **Atomicity**: Crashes may leave partial state
5. **Concurrency**: Not designed for parallel execution

## Extension Points

### Future Enhancements (Not Implemented)

#### 1. Input Fingerprinting
Track where data comes from:
```rust
inputs: vec![
    InputFingerprint {
        source: "https://api.example.com/data",
        hash: "...",
        captured_at: timestamp,
        schema: Some("json"),
    }
]
```

#### 2. Declarative Scheduling
Instead of cron:
```yaml
trigger:
  on_change: source_data
  after: other_pipeline
  manual: true
```

#### 3. Diff Operations
```bash
mirror diff <id1> <id2>
```

Compare two reflections:
- Pipeline changes
- Input changes
- Output differences

#### 4. Git Integration
```rust
transform: TransformWitness {
    content_hash: "...",
    version: Some("git:abc123"),
    source_path: "pipelines/cashflow.nu",
}
```

#### 5. Cryptographic Signatures
```rust
execution: ExecutionMeta {
    witness: "user@example.com",
    signature: Some("ed25519:..."),
}
```

#### 6. Distributed Ledger
Replicate across machines:
```
machine-1/mirror-ledger/  →  sync  →  machine-2/mirror-ledger/
```

## Performance Characteristics

### Time Complexity
- Execute pipeline: O(pipeline_time)
- Append to ledger: O(1) amortized
- List recent: O(n) where n = total reflections
- Get reflection: O(1) with ID

### Space Complexity
- Ledger grows: O(reflections)
- Reflections: O(reflections)
- Artifacts: O(unique_outputs) (deduplicated)

### Bottlenecks
1. **Disk I/O**: Append to JSONL
2. **Hashing**: SHA256 of outputs
3. **File copying**: Artifacts to content-addressed storage

All acceptable for the target use case.

## Security Considerations

### Current Model
- Daemon runs with user permissions
- No privilege escalation
- Pipelines have same access as user

### Future Hardening
- Run daemon with minimal permissions
- Pipelines in restricted sandbox
- Network access allowlist
- File system isolation (chroot/containers)

## Testing Strategy

### Unit Tests
- Reflection envelope generation
- Hash determinism
- Ledger append operations
- Content-addressed storage

### Integration Tests
- Full pipeline execution
- Multiple reflections
- Ledger reading
- Error handling

### Property Tests
- Hash collisions (probability)
- Ledger ordering invariants
- Content addressing correctness

## Why This Works

### For LDM (Large Data Models)
- **Deterministic inference**: Pipeline = model
- **Explainable**: Every step is an artifact
- **Auditable**: Ledger preserves history
- **Composable**: Pipelines can chain

### For Enterprise
- **Compliance**: Audit trail by default
- **Reproducibility**: Hash everything
- **Transparency**: No hidden state
- **Durability**: Files > databases

### For Humans
- **Inspectable**: Standard tools (cat, jq, grep)
- **Understandable**: Simple directory structure
- **Debuggable**: Full execution context
- **Trustable**: Boring > clever

## Philosophy

This daemon embodies:
- **Witness > authority**: Observe, don't control
- **Artifacts > state**: Externalize everything
- **Diff > mutation**: Compare, don't overwrite
- **Boring > flashy**: Outlive the trends

The goal is not to be clever.

The goal is to still be running in 10 years.
