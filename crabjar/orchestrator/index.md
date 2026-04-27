# Orchestrator Module
The execution gate and ACP interface. Handles command dispatch, SSE streaming, and LLM tool-call integration via LM Studio.

**Key Files:** `src/main.rs`, `Cargo.toml`
**Dependencies:** `mirror-guard`, `axum`, `tokio`, `reqwest`
**Data Source:** `local_log` (SQLite)
**Capabilities:** Command dispatch, SSE streaming, LLM tool-call integration
