# Source Directory

Active Rust surface: `crabjar` (CLI binary) + `crabjar-config` (library) + `agent-context` (library) + codeburn pipeline (5 crates).

**Key Files:** `main.rs`, `project_loader.rs`, `state_docs.rs`, `dotfile_manager.rs`
**Workspace crates:** `crabjar-config`, `codeburn-config`, `codeburn-provider`, `codeburn-classifier`, `codeburn-pricing`, `codeburn`
**Data Source:** SQLite-backed memory, TOML config, filesystem state-docs
**Capabilities:** CLI command parsing (state-docs, knowledge store), codeburn pipeline orchestration
