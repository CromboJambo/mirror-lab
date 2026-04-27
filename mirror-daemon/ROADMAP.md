# Mirror Daemon Roadmap

## Current State (v0.1.0)

**Implemented**:
- ✅ Basic pipeline execution (Nu scripts)
- ✅ Reflection envelope generation
- ✅ Append-only ledger (JSONL)
- ✅ Content-addressed artifact storage
- ✅ CLI interface (run, list, recent, inspect, stats)
- ✅ Isolated execution environments
- ✅ Output capture and hashing
- ✅ Basic integrity guarantees

**Not Implemented**:
- ❌ Input fingerprinting
- ❌ Declarative scheduling
- ❌ Diff operations
- ❌ Git integration
- ❌ Cryptographic signatures
- ❌ Distributed replication

## Phase 1: Foundation (Current)

**Goal**: Prove the core concept works

**Deliverables**:
- [x] Working daemon
- [x] CLI client
- [x] Ledger implementation
- [x] Example pipelines
- [x] Documentation

**Success Criteria**:
- Can execute pipelines
- Preserves execution history
- Artifacts are immutable
- System is boring and reliable

## Phase 2: Input Tracking

**Goal**: Track where data comes from

**Features**:
- Input fingerprinting (hash data sources)
- Source metadata (URLs, file paths, API endpoints)
- Schema hints (JSON, CSV, etc.)
- Timestamp of capture

**Implementation**:
```rust
// Extend InputFingerprint
pub struct InputFingerprint {
    source: String,           // Where data came from
    hash: String,             // Hash of input data
    captured_at: DateTime,    // When it was captured
    schema: Option<String>,   // Format hint
    size_bytes: u64,          // Data size
}
```

**Use Cases**:
- "What data did this pipeline use?"
- "Has the source data changed?"
- "When was this data last updated?"

## Phase 3: Diff Operations

**Goal**: Compare reflections

**Features**:
- Pipeline diff (what changed in the script)
- Input diff (what changed in the data)
- Output diff (what changed in results)
- Execution diff (performance/errors)

**Commands**:
```bash
mirror diff <id1> <id2>
mirror diff last two
mirror diff pipeline cashflow.nu
```

**Implementation**:
- Structural diff for JSON metadata
- Content diff for artifacts
- Visual presentation of changes

## Phase 4: Declarative Scheduling

**Goal**: Move beyond cron to causality-based execution

**Concepts**:
- **Trigger types**:
  - `on_change: source_identifier` - Run when source changes
  - `after: pipeline_name` - Run after another pipeline succeeds
  - `manual` - Only run on explicit command
  - `interval: duration` - Time-based (last resort)

**Pipeline Metadata Format**:
```yaml
# pipelines/cashflow.meta.yaml
name: cashflow
script: cashflow.nu
triggers:
  - on_change: 
      source: "s3://bucket/data.csv"
      check_interval: 5m
  - after: data_validation
inputs:
  - name: sales_data
    source: "s3://bucket/data.csv"
    schema: csv
```

**Implementation**:
- Daemon watches for triggers
- Maintains dependency graph
- Schedules based on causality
- No arbitrary timestamps

**Why This Matters**:
- "Run when data changes" not "run at 3am"
- Natural expression of dependencies
- Easier to reason about

## Phase 5: Git Integration

**Goal**: Track pipeline versions

**Features**:
- Capture git commit hash
- Link reflections to pipeline versions
- Show pipeline evolution over time
- Diff pipelines across commits

**Envelope Extension**:
```rust
pub struct TransformWitness {
    content_hash: String,
    source_path: PathBuf,
    version: Option<GitVersion>,  // New field
}

pub struct GitVersion {
    commit_hash: String,
    branch: String,
    dirty: bool,  // Uncommitted changes?
    remote_url: Option<String>,
}
```

**Benefits**:
- "What version of the pipeline produced this?"
- "How has this pipeline changed?"
- Reproducibility across machines

## Phase 6: Cryptographic Signatures

**Goal**: Prove who witnessed what

**Features**:
- Sign reflections with private keys
- Verify signatures
- Support multiple signers
- Timestamp signing

**Implementation**:
```rust
pub struct ExecutionMeta {
    exit_code: i32,
    stdout: String,
    stderr: String,
    duration_ms: u64,
    witness: String,
    signature: Option<WitnessSignature>,  // New field
}

pub struct WitnessSignature {
    algorithm: String,     // "ed25519"
    public_key: String,    // Base64 encoded
    signature: String,     // Base64 encoded
    signed_at: DateTime,
}
```

**Use Cases**:
- Multi-party computation
- Audit requirements
- Compliance (SOC2, etc.)
- Trust boundaries

## Phase 7: Advanced Querying

**Goal**: Query ledger efficiently

**Features**:
- Index by pipeline name
- Index by timestamp range
- Index by success/failure
- Index by input/output hashes

**Implementation Options**:
1. **SQLite overlay** (read-only view of JSONL)
2. **In-memory index** (rebuild on daemon start)
3. **Secondary index files** (updated on append)

**Queries**:
```bash
mirror query --pipeline cashflow.nu --success true --since 2025-01-01
mirror query --input-hash abc123 --output-hash def456
mirror query --duration-min 1000 --duration-max 5000
```

## Phase 8: Distributed Ledger

**Goal**: Replicate across machines

**Features**:
- Sync ledger between nodes
- Conflict-free replication (CRDTs)
- Partial sync (by pipeline, time range)
- Verify integrity across nodes

**Architecture**:
```
Machine 1                 Machine 2
┌─────────────┐          ┌─────────────┐
│   Ledger    │◄────────►│   Ledger    │
│             │   sync   │             │
└─────────────┘          └─────────────┘
```

**Protocol**:
- Push: Send new reflections to peers
- Pull: Request missing reflections
- Verify: Check hashes match
- Merge: Append-only, no conflicts

**Use Cases**:
- Multi-datacenter
- Backup/archival
- Collaborative teams

## Phase 9: Sandboxing

**Goal**: Run untrusted pipelines safely

**Features**:
- Container isolation (Docker/Podman)
- Resource limits (CPU, memory, time)
- Network restrictions
- File system isolation

**Configuration**:
```yaml
sandbox:
  enabled: true
  container: docker
  image: mirror-runtime:latest
  limits:
    cpu: "1.0"
    memory: "1G"
    timeout: "10m"
  network: deny
  volumes:
    - "/data:/data:ro"  # Read-only data mount
```

**Security Model**:
- Daemon runs as limited user
- Pipelines run in containers
- No privilege escalation
- Audit all resource access

## Phase 10: LDM Integration

**Goal**: Bridge to Large Data Models

**Concepts**:
- Reflections as training data
- Pipeline chains as inference paths
- Artifacts as external memory
- Ledger as reasoning trace

**Integration Points**:
1. **Export reflections to LDM format**
2. **Import LDM predictions as pipelines**
3. **Use ledger for provenance tracking**
4. **Link to embedding databases**

**Example Flow**:
```
Historical reflections
    ↓
Train LDM on patterns
    ↓
Generate new pipeline
    ↓
Execute via daemon
    ↓
Validate against expectations
    ↓
Add to ledger
```

## Non-Goals

Things we will **NOT** do:

1. **Real-time processing**: This is batch-oriented
2. **Streaming data**: Use Apache Kafka instead
3. **Complex orchestration**: Use Airflow/Prefect instead
4. **Machine learning**: This is infrastructure, not ML
5. **Web UI**: CLI and files are enough
6. **Plugin system**: Keep it simple
7. **Custom DSL**: Nu is the DSL

## Timeline

**Phase 1**: Complete ✅
**Phase 2**: 2-3 weeks (Input tracking)
**Phase 3**: 1-2 weeks (Diff operations)
**Phase 4**: 3-4 weeks (Declarative scheduling)
**Phase 5**: 1 week (Git integration)
**Phase 6**: 2 weeks (Signatures)
**Phase 7**: 2-3 weeks (Querying)
**Phase 8**: 4-6 weeks (Distribution)
**Phase 9**: 2-3 weeks (Sandboxing)
**Phase 10**: Ongoing (LDM integration)

Total to Phase 9: ~4-6 months

## Success Metrics

**Technical**:
- Zero data loss (append-only guarantees)
- < 100ms append latency
- Handles 1M+ reflections
- < 5 minutes to rebuild indexes

**Adoption**:
- Used in production by 10+ people
- 1000+ pipelines executed
- 10,000+ reflections in ledger
- Survives 1+ year in production

**Quality**:
- No critical bugs
- Documentation coverage > 80%
- Test coverage > 70%
- Clean audit trail for compliance

## Decision Log

**Why Rust?**
- Memory safety
- Performance
- Ecosystem (serde, sha2, etc.)
- Long-term stability

**Why JSONL for ledger?**
- Human-readable
- Append-friendly
- Tool-compatible (jq, grep)
- Simple to implement

**Why not use a database?**
- Files outlive databases
- Simpler to backup/replicate
- No schema migrations
- Easier to debug

**Why content-addressed storage?**
- Natural deduplication
- Integrity by design
- Immutable by construction

**Why Nu for pipelines?**
- Data-first language
- Structured types
- Shell ergonomics
- Growing ecosystem

## Contributing

This roadmap is not prescriptive. If you want to:
- Implement a phase out of order
- Add features not listed
- Change priorities

That's fine. The key principles are:
1. **Boring over clever**
2. **Durable over trendy**
3. **Simple over complex**

If a change violates these, it won't be merged.

## Questions?

Open an issue or discussion. This is experimental software.

The goal is to build something that outlasts the hype.
