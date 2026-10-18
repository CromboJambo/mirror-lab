# Mirror-Log MCP Server

JSON-RPC 2.0 over stdio transport exposing mirror-log's attention layer and event store as MCP tools. Designed for fleet-scale observation processing with crabjar-conductor's hybrid attention + JEV decision pipeline.

## Build

```bash
cargo build -p mirror-log-mcp
```

Binary: `target/debug/mirror-log-mcp` (or `target/release/` with `--release`)

## Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `MIRROR_LOG_DB_PATH` | `~/.hermes/mirror-log.db` | SQLite database path |

## Tools

### Observation Management
- `log_observation` — Record new observation with metadata
- `get_observation` — Retrieve specific observation by ID
- `list_observations` — Query recent observations (optional source filter)
- `update_observation` — Modify existing observation
- `delete_observation` — Remove observation

### Attention Operations
- `calculate_attention_score` — Compute attention score for observation
- `get_high_attention` — List observations above threshold
- `get_attention_stats` — Fleet-wide attention statistics
- `track_access` — Record access event (affects decay)
- `pin_observation` — Prevent decay for critical items
- `unpin_observation` — Restore normal decay behavior
- `decay_all` — Force global decay cycle

### Event Management
- `log_event` — Record event with type and payload
- `get_events` — Query events by type/time range

### Fleet Sync
- `sync_fleet_state` — Aggregate attention across fleet nodes
- `get_fleet_high_attention` — Cross-node high-attention items

## Integration with Crabjar Conductor

The crabjar-conductor hybrid layer (`src/attention/mod.rs`) uses direct SQLite connections for hot-path attention scoring (~135µs). This MCP server provides an optional transport layer for:

- Cold-path operations (bulk queries, historical analysis)
- Cross-process/machine communication
- Fleet persistence via gpui/herdr
- Integration with non-Rust agents

Crabjar conductor detects MCP availability via `MIRROR_LOG_MCP_URL` env var or config flag, falling back to direct SQLite if not running.

## Protocol

Standard JSON-RPC 2.0 over stdio:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}' | ./target/debug/mirror-log-mcp
```