# Getting Started with Mirror Daemon

## Quick Start

### 1. Build the daemon
```bash
cd mirror-daemon
cargo build --release
```

### 2. Set up directories
```bash
mkdir -p mirror-ledger pipelines
```

### 3. Create your first pipeline

Create `pipelines/hello.nu`:
```nu
#!/usr/bin/env nu
print "Hello from Mirror!"

let data = {
    message: "This is a reflection",
    timestamp: (date now | format date "%Y-%m-%d %H:%M:%S")
}

$data | to json | save greeting.json
```

### 4. Run it
```bash
./target/release/mirror run hello.nu
```

You'll see output like:
```
Running pipeline: hello.nu
Witness: cli

Reflection sealed: abc123def456...

Execution summary:
  Exit code: 0
  Duration: 45ms
  Outputs: 1

Stdout:
Hello from Mirror!
```

### 5. Inspect the results

View recent executions:
```bash
./target/release/mirror recent
```

Inspect the specific reflection:
```bash
./target/release/mirror inspect abc123def456...
```

View ledger stats:
```bash
./target/release/mirror stats
```

## Understanding What Happened

When you ran `mirror run hello.nu`, the daemon:

1. **Read** the pipeline: `pipelines/hello.nu`
2. **Hashed** its content: SHA256 of the script
3. **Executed** it in isolation: Created a temporary work directory
4. **Captured** everything:
   - Exit code
   - stdout/stderr
   - Duration
   - All generated files
5. **Sealed** a reflection envelope with:
   - Timestamp
   - Pipeline hash
   - Input fingerprints (empty for now)
   - Output artifacts (greeting.json)
   - Execution metadata
6. **Appended** to the ledger: One line in `ledger.jsonl`

## Exploring the Ledger

Look at the directory structure:
```bash
tree mirror-ledger/
```

You'll see:
```
mirror-ledger/
├── ledger.jsonl              # One line per execution
├── reflections/              # Sealed envelopes
│   └── ab/
│       └── abc123.../
│           └── meta.json
├── artifacts/                # Content-addressed outputs
│   └── de/
│       └── def456...
└── work/                     # Temporary execution spaces
```

Read the ledger directly:
```bash
cat mirror-ledger/ledger.jsonl | jq
```

Each line is a ledger entry:
```json
{
  "reflection_id": "abc123...",
  "ledger_time": "2025-02-01T12:00:00Z",
  "envelope_path": "reflections/ab/abc123.../meta.json",
  "pipeline": "hello.nu",
  "success": true
}
```

View a reflection envelope:
```bash
cat mirror-ledger/reflections/ab/abc123.../meta.json | jq
```

## Key Concepts

### Nothing is Ever Deleted

The ledger is **append-only**. Every execution is preserved forever.

If you run the same pipeline 10 times, you'll have 10 reflections.

### Content-Addressed Storage

Artifacts are stored by their hash. If two pipelines generate identical output, they share the same artifact.

### Deterministic Hashing

Same pipeline + same inputs = same hash (eventually, with input tracking)

### No Hidden State

Everything is:
- A file on disk
- JSON metadata
- Inspectable with standard tools

### Boring is Good

No database. No API. No magic.

Just:
- Files
- Directories
- Hashes
- Timestamps

## Common Workflows

### Run a pipeline and save the ID
```bash
REFLECTION_ID=$(./target/release/mirror run analysis.nu | grep "sealed:" | cut -d' ' -f3)
```

### Compare two reflections (manual for now)
```bash
diff \
  mirror-ledger/reflections/ab/abc123.../meta.json \
  mirror-ledger/reflections/cd/cde456.../meta.json
```

### Find all successful executions
```bash
cat mirror-ledger/ledger.jsonl | jq 'select(.success == true)'
```

### Track a specific pipeline over time
```bash
cat mirror-ledger/ledger.jsonl | jq 'select(.pipeline == "cashflow.nu")'
```

## Next Steps

Try the example pipelines:
```bash
cp examples/pipelines/* pipelines/
./target/release/mirror list
./target/release/mirror run cashflow.nu
./target/release/mirror run sales_analysis.nu
./target/release/mirror run health_check.nu
```

Examine the ledger:
```bash
./target/release/mirror recent --limit 5
./target/release/mirror stats
```

Create your own pipeline:
```bash
cat > pipelines/my_report.nu << 'EOF'
#!/usr/bin/env nu
# Your pipeline here
print "Building my report..."

let report = {
    title: "My Report",
    data: [1, 2, 3, 4, 5]
}

$report | to json | save my_report.json
EOF

./target/release/mirror run my_report.nu
```

## Philosophy Reminder

This tool is intentionally:
- **Boring**: No clever features
- **Strict**: No flexibility that breaks guarantees
- **Lawful**: Rules are enforced, not suggested
- **Dumb**: Does exactly what you tell it

This is a feature, not a limitation.

The goal is to outlive everything flashier.
