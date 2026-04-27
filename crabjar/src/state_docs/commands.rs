// crabjar/src/state_docs/commands.rs
// CLI command module for state-docs: index, show, query, list

use agent_context::state_docs::models::{
    Annotation, CodeBlock, ConfidenceAssessment, DocMetadata, Section, Table,
};
use agent_context::state_docs::{MarkdownIndexer, StateDocQuerier, ZoomRenderer};
use clap::{Args, Subcommand};
use rusqlite::Connection;
use serde_json::json;
use std::path::PathBuf;

/// CLI commands for state-docs management
#[derive(Debug, Subcommand, Clone)]
pub enum StateDocsCommand {
    /// Index all state-docs into SQLite
    Index {
        /// Path to the state-docs directory (defaults to ./state-docs)
        #[arg(long, default_value = "state-docs")]
        docs_dir: String,

        /// Path to the SQLite database (defaults to ./state-docs.db)
        #[arg(long, default_value = "state-docs.db")]
        db_path: String,
    },

    /// Show a state-doc with configurable zoom depth
    Show {
        /// State-doc name (e.g., claw-code-state)
        doc_name: String,

        /// Zoom level: 1=overview, 2=section, 3=paragraph
        #[arg(long, default_value_t = 2)]
        zoom: u8,
    },

    /// Query a state-doc by section or keyword
    Query {
        /// State-doc name
        doc_name: String,

        /// Section name to query (mutually exclusive with keyword)
        #[arg(long)]
        section: Option<String>,

        /// Keyword to search across sections (mutually exclusive with section)
        #[arg(long)]
        keyword: Option<String>,

        /// SQLite database path
        #[arg(long, default_value = "state-docs.db")]
        db_path: String,
    },

    /// List all indexed state-docs
    List {
        /// SQLite database path
        #[arg(long, default_value = "state-docs.db")]
        db_path: String,
    },

    /// Get confidence assessment for a state-doc
    Confidence {
        /// State-doc name
        doc_name: String,

        /// SQLite database path
        #[arg(long, default_value = "state-docs.db")]
        db_path: String,
    },

    /// Get annotations for a state-doc
    Annotations {
        /// State-doc name
        doc_name: String,

        /// SQLite database path
        #[arg(long, default_value = "state-docs.db")]
        db_path: String,
    },

    /// Get tables extracted from a state-doc
    Tables {
        /// State-doc name
        doc_name: String,

        /// SQLite database path
        #[arg(long, default_value = "state-docs.db")]
        db_path: String,
    },

    /// Get code blocks extracted from a state-doc
    CodeBlocks {
        /// State-doc name
        doc_name: String,

        /// SQLite database path
        #[arg(long, default_value = "state-docs.db")]
        db_path: String,
    },
}

impl StateDocsCommand {
    /// Execute the command
    pub fn execute(&self) -> Result<serde_json::Value, StateDocsError> {
        match self {
            Self::Index { docs_dir, db_path } => {
                let conn = Connection::open(db_path)?;
                let count = MarkdownIndexer::index_all(&conn, docs_dir)?;
                Ok(json!({
                    "success": true,
                    "message": format!("indexed {} state-docs", count),
                    "payload": {
                        "count": count,
                        "docs_dir": docs_dir,
                        "db_path": db_path,
                    }
                }))
            }

            Self::Show { doc_name, zoom } => {
                let conn = Connection::open("state-docs.db")?;
                let (markdown, metadata) = ZoomRenderer::render(&conn, doc_name, *zoom)?;
                Ok(json!({
                    "success": true,
                    "message": format!("rendered {} at zoom level {}", doc_name, zoom),
                    "payload": {
                        "doc": doc_name,
                        "zoom": zoom,
                        "markdown": markdown,
                        "metadata": metadata,
                    }
                }))
            }

            Self::Query {
                doc_name,
                section,
                keyword,
                db_path,
            } => {
                let conn = Connection::open(db_path)?;
                let querier = StateDocQuerier::new(conn, PathBuf::from(db_path));

                if let Some(section) = section {
                    let result = querier.query_by_section(doc_name, &section);
                    Ok(json!({
                        "success": true,
                        "message": format!("queried section '{}' in {}", section, doc_name),
                        "payload": result,
                    }))
                } else if let Some(keyword) = keyword {
                    let result = querier.query_by_keyword(doc_name, &keyword);
                    Ok(json!({
                        "success": true,
                        "message": format!("searched keyword '{}' in {}", keyword, doc_name),
                        "payload": result,
                    }))
                } else {
                    Err(StateDocsError::InvalidInput(
                        "must provide --section or --keyword".to_string(),
                    ))
                }
            }

            Self::List { db_path } => {
                let conn = Connection::open(db_path)?;
                let docs = querier_list_docs(&conn)?;
                Ok(json!({
                    "success": true,
                    "message": format!("listed {} state-docs", docs.len()),
                    "payload": {
                        "docs": docs,
                    }
                }))
            }

            Self::Confidence { doc_name, db_path } => {
                let conn = Connection::open(db_path)?;
                let assessment = get_confidence(&conn, doc_name)?;
                Ok(json!({
                    "success": true,
                    "message": format!("retrieved confidence for {}", doc_name),
                    "payload": {
                        "doc": doc_name,
                        "confidence": assessment,
                    }
                }))
            }

            Self::Annotations { doc_name, db_path } => {
                let conn = Connection::open(db_path)?;
                let annotations = get_annotations(&conn, doc_name)?;
                let open_count = annotations.iter().filter(|a| a.status == "open").count();
                Ok(json!({
                    "success": true,
                    "message": format!("retrieved {} annotations for {}", annotations.len(), doc_name),
                    "payload": {
                        "doc": doc_name,
                        "annotations": annotations,
                        "open_count": open_count,
                    }
                }))
            }

            Self::Tables { doc_name, db_path } => {
                let conn = Connection::open(db_path)?;
                let tables = get_tables(&conn, doc_name)?;
                Ok(json!({
                    "success": true,
                    "message": format!("retrieved {} tables for {}", tables.len(), doc_name),
                    "payload": {
                        "doc": doc_name,
                        "tables": tables,
                    }
                }))
            }

            Self::CodeBlocks { doc_name, db_path } => {
                let conn = Connection::open(db_path)?;
                let code_blocks = get_code_blocks(&conn, doc_name)?;
                Ok(json!({
                    "success": true,
                    "message": format!("retrieved {} code blocks for {}", code_blocks.len(), doc_name),
                    "payload": {
                        "doc": doc_name,
                        "code_blocks": code_blocks,
                    }
                }))
            }
        }
    }
}

/// Helper to produce a standard state-docs response JSON object
fn state_docs_response(
    message: impl Into<String>,
    payload: serde_json::Value,
) -> serde_json::Value {
    json!({
        "success": true,
        "message": message.into(),
        "payload": payload,
    })
}

/// List all indexed state-docs from SQLite
fn querier_list_docs(conn: &Connection) -> Result<Vec<serde_json::Value>, StateDocsError> {
    let mut stmt = conn.prepare(
        "SELECT name, description, last_modified, line_count, section_count, table_count, code_block_count, annotation_count, open_annotation_count FROM doc_metadata ORDER BY last_modified DESC"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(json!({
            "name": row.get(0)?,
            "description": row.get(1)?,
            "last_modified": row.get(2)?,
            "line_count": row.get(3)?,
            "section_count": row.get(4)?,
            "table_count": row.get(5)?,
            "code_block_count": row.get(6)?,
            "annotation_count": row.get(7)?,
            "open_annotation_count": row.get(8)?,
        }))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Get confidence assessment for a state-doc
fn get_confidence(conn: &Connection, doc_name: &str) -> Result<serde_json::Value, StateDocsError> {
    let mut stmt = conn.prepare(
        "SELECT what_captured, what_missed, assumptions, blind_spots, stale_after FROM confidence WHERE doc_name = ?1"
    )?;

    let row = stmt
        .query_row(rusqlite::params![doc_name], |row| {
            Ok(json!({
                "what_captured": row.get(0)?,
                "what_missed": row.get(1)?,
                "assumptions": row.get(2)?,
                "blind_spots": row.get(3)?,
                "stale_after": row.get(4)?,
            }))
        })
        .map_err(|e| {
            if e == rusqlite::Error::QueryReturnedNoRows {
                StateDocsError::NotFound(format!("no confidence assessment for {}", doc_name))
            } else {
                StateDocsError::Database(e)
            }
        })?;

    Ok(row)
}

/// Get annotations for a state-doc
fn get_annotations(
    conn: &Connection,
    doc_name: &str,
) -> Result<Vec<serde_json::Value>, StateDocsError> {
    let mut stmt = conn.prepare(
        "SELECT line, kind, message, author, status, created_at FROM annotations WHERE doc_name = ?1 ORDER BY line ASC"
    )?;

    let rows = stmt.query_map(rusqlite::params![doc_name], |row| {
        Ok(json!({
            "line": row.get(0)?,
            "kind": row.get(1)?,
            "message": row.get(2)?,
            "author": row.get(3)?,
            "status": row.get(4)?,
            "created_at": row.get(5)?,
        }))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Get tables for a state-doc
fn get_tables(conn: &Connection, doc_name: &str) -> Result<Vec<serde_json::Value>, StateDocsError> {
    let mut stmt = conn.prepare(
        "SELECT start_line, end_line, headers, rows FROM tables WHERE doc_name = ?1 ORDER BY start_line ASC"
    )?;

    let rows = stmt.query_map(rusqlite::params![doc_name], |row| {
        Ok(json!({
            "start_line": row.get(0)?,
            "end_line": row.get(1)?,
            "headers": row.get(2)?,
            "rows": row.get(3)?,
        }))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Get code blocks for a state-doc
fn get_code_blocks(
    conn: &Connection,
    doc_name: &str,
) -> Result<Vec<serde_json::Value>, StateDocsError> {
    let mut stmt = conn.prepare(
        "SELECT start_line, end_line, language, line_count FROM code_blocks WHERE doc_name = ?1 ORDER BY start_line ASC"
    )?;

    let rows = stmt.query_map(rusqlite::params![doc_name], |row| {
        Ok(json!({
            "start_line": row.get(0)?,
            "end_line": row.get(1)?,
            "language": row.get(2)?,
            "line_count": row.get(3)?,
        }))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Errors for state-docs CLI commands
#[derive(Debug, thiserror::Error)]
pub enum StateDocsError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Indexing error: {0}")]
    Indexing(String),
}
