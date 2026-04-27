# mirror-query

**Local AI decompression layer for mirror-log**

Query your append-only event log with natural language using local AI. No cloud, no API keys, your data stays yours.

## Philosophy

This is the **decompression pattern** in action:

```
Your Data (Compressed)          Query Interface (Decompression)
┌─────────────────────┐        ┌──────────────────────┐
│  mirror.db          │   →    │  mirror-query        │
│  (append-only log)  │        │  (local AI)          │
│                     │        │                      │
│  - Stable           │        │  - Generates views   │
│  - Immutable        │        │  - No storage        │
│  - Debuggable       │        │  - Infinite queries  │
└─────────────────────┘        └──────────────────────┘
```

**Key insight:** The same log supports infinite queries without storing the results. True decompression.

## What This Is

- ✅ Local-first AI querying
- ✅ Works offline (no API calls)
- ✅ Privacy-preserving (data never leaves your machine)
- ✅ Stateless (nothing stored, pure decompression)
- ✅ Composable (works with mirror-log, rsf-cli, etc.)

## What This Is NOT

- ❌ A vector database
- ❌ An embeddings store
- ❌ A RAG system (yet - that's a future layer)
- ❌ Cloud-dependent
- ❌ Another ChatGPT wrapper

## Prerequisites

### 1. Mirror-log database

You need a `mirror.db` file from [mirror-log](https://github.com/CromboJambo/mirror-log):

```bash
# Install mirror-log
git clone https://github.com/CromboJambo/mirror-log
cd mirror-log
cargo install --path .

# Add some events
mirror-log add "First thought about AI scaffolding" --source journal
mirror-log add "Built rsf-cli today" --source dev-log
mirror-log add "Need to integrate with local AI" --source ideas
```

### 2. Ollama (local AI)

Install [Ollama](https://ollama.ai) for local AI:

```bash
# Install Ollama
curl -fsSL https://ollama.ai/install.sh | sh

# Pull a model (llama3.2 is fast and good)
ollama pull llama3.2

# Start the server
ollama serve
```

## Installation

```bash
git clone <repo-url>
cd mirror-query
cargo build --release
cargo install --path .
```

The binary will be at `target/release/mirror-query` or installed to `~/.cargo/bin/mirror-query`.

## Usage

### Basic Query

```bash
# Simple question
mirror-query "What have I been thinking about lately?"

# Query with context window
mirror-query "Summarize my development work" --context 100

# Filter by source
mirror-query "What are my journal insights?" --source journal
```

### Output Formats

```bash
# Default: plain text
mirror-query "What patterns do you see?"

# JSON output
mirror-query "Summarize" --format json

# Markdown output
mirror-query "Create a report" --format markdown
```

### Advanced Options

```bash
# Use different model
mirror-query "Analyze my code notes" --model codellama

# Different database
mirror-query "What's in here?" --db /path/to/other.db

# Debug mode (see what's happening)
mirror-query "Test query" --debug
```

## How It Works

1. **Fetches context** - Reads recent events from mirror.db (default: last 50)
2. **Builds prompt** - Combines events with your question
3. **Queries local AI** - Sends to Ollama running on your machine
4. **Returns answer** - Prints result (nothing stored)

**Crucially:** No results are stored. Every query is fresh decompression from the source.

## Examples

### Pattern Recognition

```bash
$ mirror-query "What themes appear most in my journal?"

Based on your recent journal entries, the main themes are:
1. Building local-first tools (mentioned 8 times)
2. Community gardens vs walled gardens (6 times)
3. Stable scaffolding patterns (5 times)
```

### Timeline Generation

```bash
$ mirror-query "Create a timeline of my Rust learning" --format markdown

# Rust Learning Timeline

## Week 1 (Jan 1-7)
- Started with basic syntax
- Struggled with borrowing

## Week 2 (Jan 8-14)
- Built first CLI tool
- Understanding ownership better
...
```

### Code Session Analysis

```bash
$ mirror-query "What did I work on last week?" --source dev-log

Last week you worked on:
- rsf-cli: Added streaming support for large files
- mirror-log: Improved chunking algorithm
- Fixed bug in cardinality ranking
```

## Integration with Other Tools

### With RSF

```bash
# Export events as CSV
sqlite3 mirror.db "SELECT * FROM events" > events.csv

# Rank with rsf-cli
rsf rank events.csv -o events.rsf

# Query the ranked data
mirror-query "Explain the cardinality ranking in events.rsf"
```

### With Scripts

```bash
#!/bin/bash
# Daily summary script

DATE=$(date +%Y-%m-%d)
SUMMARY=$(mirror-query "Summarize today's events" --format markdown)

echo "$SUMMARY" > daily-summaries/$DATE.md
```

## Privacy & Security

### What Stays Local

- ✅ Your `mirror.db` file (never uploaded)
- ✅ All events and content (never sent to cloud)
- ✅ AI model (runs on your machine via Ollama)
- ✅ Query results (generated locally, not stored)

### What Leaves Your Machine

- ❌ Nothing (if using Ollama locally)

### If You Don't Trust Ollama

You can:
1. Audit the Ollama source code (it's open source)
2. Run it in a network-isolated environment
3. Use a different local AI backend (future: llama.cpp support)

## Performance

**Fast queries:**
- Context: 50 events
- Model: llama3.2
- Response time: ~2-5 seconds

**Large queries:**
- Context: 500 events
- Model: llama3.2
- Response time: ~10-20 seconds

**Tip:** Use `--context` to control speed vs comprehensiveness tradeoff.

## Comparison

| Feature | Mirror-Query | ChatGPT/Claude | RAG Systems |
|---------|--------------|----------------|-------------|
| Local AI | ✅ | ❌ | Varies |
| Privacy | ✅ | ❌ | Varies |
| Offline | ✅ | ❌ | ❌ |
| No API costs | ✅ | ❌ | ❌ |
| Vector DB needed | ❌ | N/A | ✅ |
| Stateless | ✅ | N/A | ❌ |

## Troubleshooting

### "Failed to connect to Ollama"

```bash
# Start Ollama server
ollama serve

# In another terminal
mirror-query "test"
```

### "No events found in database"

```bash
# Make sure mirror.db exists
ls -la mirror.db

# Add some events
mirror-log add "Test event" --source test
```

### Slow responses

```bash
# Reduce context size
mirror-query "question" --context 20

# Use a smaller/faster model
ollama pull llama3.2:1b
mirror-query "question" --model llama3.2:1b
```

## Future Extensions

Possible future layers (separate tools or flags):

- **Embeddings support** - For semantic similarity search
- **Chunk-aware querying** - Use mirror-log's chunk table
- **Query caching** - Optional result storage
- **Multi-turn conversations** - Maintain context across queries
- **Custom prompt templates** - For different query types
- **llama.cpp support** - Alternative to Ollama

But v0.1 is intentionally minimal. Prove the decompression pattern first.

## Philosophy: Why This Matters

### The Problem

Current AI tools force you to choose:
- **Cloud AI** - Fast but privacy nightmare
- **RAG systems** - Complex vector DBs, embeddings, storage
- **Context stuffing** - Copy-paste into ChatGPT manually

### The Solution

**Decompression, not storage:**

Your mirror-log is the compressed source of truth. Mirror-query decompresses it on-demand into whatever perspective you need, then discards the result.

Same log → Infinite queries → Zero storage overhead

### The Bigger Vision

This is **stable scaffolding for the local AI era**:

```
When local AI becomes ubiquitous, you'll want:
1. Your data in stable formats (SQLite, CSV, append-only logs)
2. Query interfaces that decompress on-demand
3. No vendor lock-in
4. Complete privacy
5. Offline capability

Mirror-query proves this pattern works.
```

## Related Projects

Part of the mirror-log ecosystem:

- [mirror-log](https://github.com/CromboJambo/mirror-log) - Append-only event storage
- [rsf-cli](https://github.com/CromboJambo/rsf-cli) - Ranked spreadsheet format

All building **stable scaffolding on stable platforms**.

## License

AGPL-3.0-or-later

Like mirror-log and rsf-cli, this ensures that if anyone runs a modified version as a network service, they must make the source available.

## Credits

Built as part of the mirror-log ecosystem - stable scaffolding for personal knowledge in the age of local AI.

Inspired by:
- The XKCD "Dependency" meme
- Community gardens, not walled gardens
- Decompression as a first-class pattern
- Friction-aware automation

**We're building tiny stable pieces that hold up the future.** 🏗️
