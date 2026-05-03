# mirror-lab

**mirror-lab** is a Rust workspace for a personal knowledge-management system. It ingests events, stores them in an append-only SQLite log, supports semantic chunking and local AI querying, and provides a voice/TTS interface.

## Integration Roadmap

The project is currently undergoing a consolidation phase to merge experimental crates (`ingress`, clipboard-watching, legacy CLI tooling) into the core `mirror-*` ecosystem.

### Phase 1: Standardization
- **Dependency Alignment**: Migrating common dependencies (e.g., `tokio`, `thiserror`) to the workspace root.
- **Unified Error Handling**: Enforcing `thiserror` for libraries and `anyhow` for binaries.
- **CI/CD Readiness**: Ensuring all members pass unified linting and formatting checks.

### Phase 2: Feature Integration
- **Ingress Expansion**: Moving `ingress` logic into `mirror-daemon`.
- **Event Source Expansion**: Integrating clipboard-watching as a watcher in `mirror-daemon`.
- **UI/CLI Convergence**: Converging legacy CLI tooling into `mirror-query` and new high-level interfaces.

### Phase 3: Consolidation
- **Archive Cleanup**: Moving completed experiments to an `archive/` directory.
- **Final Workspace Polish**: A unified, single-purpose workspace structure.
