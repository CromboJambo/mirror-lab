//! mirror-guard: Annealing knowledge distillation, trust-layer retrieval, and execution gating.
//!
//! Provides the action-gating layer for the mirror-lab workspace, maintaining
//! strict separation between detection (mirror-log) and authorization (mirror-guard).
//!
//! ## Core Components
//!
//! - **Memory Graph** (`memory`): Nodes and edges representing knowledge structures
//! - **Trust Layers** (`trust`): Confidence bands that determine auto-execute behavior
//! - **Annealing Pipeline** (`annealing`): Iterative confidence decay and reinforcement
//! - **Retrieval Engine** (`retrieval`): Layer-based querying with trust filtering
//! - **Execution Gate** (`gate`): The single point where detection becomes authorized action
//!
//! ## Architecture
//!
//! ```text
//! mirror-log (detection)  ──events──>  mirror-guard (authorization)  ──gated──>  mirror-daemon (action)
//!     append-only                  separate DB (guard.db)                  execution gate
//! ```
//!
//! ## Key Principles
//!
//! - **Detection != Authorization**: Knowing what happened doesn't grant the right to act
//! - **Confidence Decay**: Patterns decay over time unless reinforced by success
//! - **Every Abstraction Carries Doubt**: Outputs include uncertainty, assumptions, and staleness info

pub mod annealing;
pub mod gate;
pub mod guard_db;
pub mod memory;
pub mod retrieval;
pub mod trust;
pub mod types;

pub use annealing::AnnealingPipeline;
pub use gate::{CommandRisk, ExecutionGate, GateContext, GateResult};
pub use guard_db::{GuardDb, GuardDbError};
pub use memory::MemoryGraph;
pub use retrieval::RetrievalEngine;
pub use trust::TrustManager;
pub use types::*;
