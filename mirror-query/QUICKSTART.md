# Mirror-Query Quick Start

## Prerequisites Check

```bash
# 1. Do you have Ollama installed?
ollama --version

# If not:
curl -fsSL https://ollama.ai/install.sh | sh

# 2. Do you have a model?
ollama list

# If not, pull one:
ollama pull llama3.2

# 3. Is Ollama running?
# In one terminal:
ollama serve

# 4. Do you have mirror.db?
ls -la mirror.db

# If not, create some events:
mirror-log add "Testing mirror-query integration" --source test
mirror-log add "Building local AI scaffolding" --source journal
mirror-log add "Rust is fun but borrow checker is hard" --source dev-log
```

## First Query

```bash
# Build and run
cargo run -- "What's in my database?"

# Or if installed:
mirror-query "What's in my database?"
```

Expected output:
```
Your database contains events from different sources including test, journal, 
and dev-log. The most recent entries discuss mirror-query integration and 
Rust development experiences.
```

## Try Different Queries

```bash
# Pattern recognition
mirror-query "What am I thinking about most?"

# Summarization
mirror-query "Summarize my recent thoughts"

# Specific topic
mirror-query "What have I said about Rust?"

# Filtered by source
mirror-query "Show my journal insights" --source journal

# More context
mirror-query "Give me a detailed summary" --context 100
```

## Output Formats

```bash
# JSON for scripting
mirror-query "Summarize" --format json | jq '.response'

# Markdown for documents
mirror-query "Create a weekly report" --format markdown > report.md

# Debug mode to see what's happening
mirror-query "Test" --debug
```

## Common Patterns

### Daily Summary Script

```bash
#!/bin/bash
# save as: daily-summary.sh

DATE=$(date +%Y-%m-%d)
mirror-query "Summarize today's events" \
  --format markdown \
  > summaries/$DATE.md

echo "Summary saved to summaries/$DATE.md"
```

### Search Your Logs

```bash
# Instead of grep, ask AI
mirror-query "Find mentions of 'scaffolding' in context"

# Semantic search (finds related concepts, not just keywords)
mirror-query "What did I write about architecture?"
```

### Generate Views

```bash
# Timeline
mirror-query "Create a timeline of my coding work this week" --source dev-log

# Categories
mirror-query "Categorize my journal entries by topic"

# Insights
mirror-query "What patterns do you see in my work habits?"
```

## Integration Examples

### With Mirror-Log

```bash
# Add event, then immediately query about it
mirror-log add "Had a breakthrough on the decompression pattern" --source eureka
mirror-query "What was my latest breakthrough?"
```

### With RSF

```bash
# Export and rank
sqlite3 mirror.db "SELECT * FROM events" > events.csv
rsf rank events.csv -o events.rsf

# Query about the ranking
mirror-query "Why are columns ranked this way in events.rsf?"
```

### In Scripts

```bash
#!/bin/bash
# Weekly digest automation

# Get summary
SUMMARY=$(mirror-query "Create a weekly digest of my work" --format markdown)

# Email it
echo "$SUMMARY" | mail -s "Weekly Digest" you@example.com

# Or save it
echo "$SUMMARY" > weekly-digests/$(date +%Y-week-%V).md
```

## Troubleshooting

### Query hangs or is slow

```bash
# Reduce context
mirror-query "question" --context 20

# Use faster model
ollama pull llama3.2:1b
mirror-query "question" --model llama3.2:1b
```

### "Connection refused"

```bash
# Make sure Ollama is running
# Terminal 1:
ollama serve

# Terminal 2:
mirror-query "test"
```

### Poor quality responses

```bash
# Try a larger model
ollama pull llama3.2:7b
mirror-query "question" --model llama3.2:7b

# Or provide more context
mirror-query "question" --context 100
```

## Next Steps

1. **Add more events** - The more context, the better the queries
2. **Experiment with models** - Try different Ollama models
3. **Build scripts** - Automate your common queries
4. **Share patterns** - What queries work well for you?

## Philosophy Reminder

This tool proves the **decompression pattern**:

- Your data: Stable, append-only, in SQLite
- Your queries: Infinite, stateless, locally generated
- Your privacy: Complete (nothing leaves your machine)
- Your control: Fork it, modify it, own it

**Community gardens, not walled gardens.** 🌱

## Getting Help

If something doesn't work:

1. Check `--debug` output
2. Verify Ollama is running: `ollama list`
3. Verify database exists: `ls -la mirror.db`
4. Try a simple query first: `mirror-query "test" --context 5`

The code is simple (~200 lines). Read it. Understand it. Modify it.

**That's the point of stable scaffolding.**
