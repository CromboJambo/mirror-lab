# Mirror-Lab

Home-lab management and observation system with attention-based memory architecture.

## Overview

Mirror-Lab provides a declarative infrastructure management layer for home-lab environments, combined with an observation system that uses temporal dynamics to prioritize relevant information. The system is designed for fleet-scale deployment across multiple machines while maintaining efficient resource utilization through natural decay of stale observations.

## Components

### `mirror-log` — Observation & Memory System
Core library providing:
- **Observation logging**: Record events with timestamps, sources, and metadata
- **Attention scoring**: Temporal dynamics-based relevance calculation (0-100 scale)
- **Decay management**: Automatic fading of stale observations
- **Shadow state**: Archival storage for decayed observations

### `mirror-log-mcp` — MCP Server
JSON-RPC 2.0 over stdio transport exposing observation operations:
- Observation CRUD (create, read, update, delete)
- Attention scoring and threshold management
- Event queries and historical analysis
- Fleet sync for cross-node aggregation

### `labctl` — CLI Tool
Declarative infrastructure management via `lab.toml` manifests:
- Machine provisioning and configuration
- Service deployment and lifecycle management
- VM orchestration
- Access rule enforcement

## Architecture

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│   labctl    │────▶│  mirror-log  │◀────│ MCP Server  │
│ (CLI tool)  │     │ (core lib)   │     │ (transport) │
└─────────────┘     └──────────────┘     └─────────────┘
       │                    │                     │
       ▼                    ▼                     ▼
┌─────────────────────────────────────────────────────┐
│              SQLite Database (mirror.db)            │
│  Tables: events, decay, observations, shadow_state  │
└─────────────────────────────────────────────────────┘
```

## Quick Start

### Building
```bash
cargo build --workspace
```

### Running Tests
```bash
cargo test --workspace
```

### Using the MCP Server
```bash
# Set environment variables
export MIRROR_LOG_DB_PATH=/path/to/mirror.db
export MIRROR_LOG_NODE_ID=node-1

# Start server
./target/release/mirror-log-mcp
```

## Attention Model

Observations are scored based on temporal dynamics:
- **Recent events** receive higher scores
- **Frequently accessed** observations maintain relevance
- **Pinned items** remain at high attention regardless of age
- **Decay** naturally reduces scores over time

This creates a self-regulating memory system where only relevant information consumes bandwidth and processing resources.

## License

MIT License — see [LICENSE](LICENSE) for details.
