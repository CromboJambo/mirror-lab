pub mod indexer;
pub mod models;
pub mod querier;
pub mod renderer;
/// state_docs — bridges Markdown state-docs with SQLite for agent queryability.
///
/// This module provides:
/// - Markdown parsing into indexed SQLite tables (sections, tables, code blocks, confidence)
/// - Query interface for agents to search state-docs by section, keyword, or annotation
/// - Zoom renderer that reconstructs Markdown from SQLite for human consumption
///
/// The schema follows the 3-level fractal pattern:
///   h1 (doc title) → h2 (section) → h3 (subsection)
/// Each level carries metadata: line range, content summary, annotations, confidence.
///
/// Usage:
/// - `indexer::index_doc()` — parse a state-doc and write to SQLite
/// - `querier::query()` — search indexed state-docs
/// - `renderer::render()` — reconstruct Markdown from indexed data
///
/// Schema tables:
/// - `doc_metadata` — doc name, description, last_modified, confidence metadata
/// - `sections` — hierarchical section tree with line ranges and summaries
/// - `tables` — extracted table data with row/column structure
/// - `code_blocks` — language, line range, content hash
/// - `confidence` — blind_spots, assumptions, stale_after, what_captured
/// - `annotations` — overlay annotations linked by line
///
/// Every abstraction carries its own doubt. The confidence table is mandatory.
pub mod schema;

pub use models::{Annotation, CodeBlock, ConfidenceAssessment, DocMetadata, Section, Table};
pub use querier::StateDocQuerier;
pub use renderer::Renderer;
pub use schema::migrate;

/// Default directory name for state-docs in a project root.
pub const STATE_DOCS_DIR: &str = "state-docs";

/// Default overlay directory name.
pub const OVERLAY_DIR: &str = "overlay";

/// Maximum heading depth for indexing (3 levels: h1 → h2 → h3).
pub const MAX_HEADING_DEPTH: usize = 3;
