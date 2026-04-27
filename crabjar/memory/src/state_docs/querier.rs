// crabjar/memory/src/state_docs/querier.rs
// Query interface for agents to search state-docs via SQLite

use crate::state_docs::models::Section;
use rusqlite::Connection;
use serde_json::json;
use std::path::PathBuf;

/// Query interface for agents to search state-docs via SQLite.
///
/// This provides a clean interface for agents to query state-docs without
/// needing to parse Markdown directly. The querier returns structured JSON
/// that agents can consume programmatically.
pub struct StateDocQuerier {
    conn: Connection,
    _state_docs_dir: PathBuf,
}

impl StateDocQuerier {
    /// Opens a querier with the given SQLite connection and state-docs directory.
    pub fn new(conn: Connection, state_docs_dir: PathBuf) -> Self {
        Self {
            conn,
            _state_docs_dir: state_docs_dir,
        }
    }

    /// Query a specific section by name within a state-doc.
    ///
    /// Returns the section content, child sections, tables, code blocks,
    /// and any annotations on that section.
    pub fn query_by_section(&self, doc_name: &str, section_name: &str) -> serde_json::Value {
        let section = self.get_section(doc_name, section_name);
        let child_sections = self.get_child_sections(doc_name, section_name);
        let tables = self.get_tables_for_section(doc_name, section_name);
        let code_blocks = self.get_code_blocks_for_section(doc_name, section_name);
        let annotations = self.get_annotations_for_section(doc_name, section_name);

        json!({
            "doc": doc_name,
            "section": section_name,
            "content_hash": section.as_ref().map(|s| s.content_hash.clone()),
            "start_line": section.as_ref().map(|s| s.start_line),
            "end_line": section.as_ref().map(|s| s.end_line),
            "child_sections": child_sections,
            "tables": tables,
            "code_blocks": code_blocks,
            "annotations": annotations,
        })
    }

    /// Query a state-doc by keyword.
    ///
    /// Returns all sections that contain the keyword, with a snippet of
    /// the matching content.
    pub fn query_by_keyword(&self, doc_name: &str, keyword: &str) -> serde_json::Value {
        let sections = self.search_sections(doc_name, keyword);

        json!({
            "doc": doc_name,
            "keyword": keyword,
            "matches": sections,
        })
    }

    /// Query all state-docs by tags.
    ///
    /// Returns docs that have any of the given tags in their metadata.
    pub fn query_by_tags(&self, tags: &[&str]) -> serde_json::Value {
        let docs = self.search_docs_by_tags(tags);

        json!({
            "tags": tags,
            "docs": docs,
        })
    }

    /// Get the confidence assessment for a state-doc.
    ///
    /// Returns the confidence section with what was captured, what was missed,
    /// assumptions, blind spots, and stale_after.
    pub fn get_confidence(&self, doc_name: &str) -> serde_json::Value {
        let confidence = self.get_confidence_assessment(doc_name);

        json!({
            "doc": doc_name,
            "confidence": confidence,
        })
    }

    /// Get all annotations for a state-doc.
    ///
    /// Returns open annotations with their section, line, kind, and message.
    pub fn get_annotations(&self, doc_name: &str) -> serde_json::Value {
        let annotations = self.get_annotations_for_doc(doc_name);

        json!({
            "doc": doc_name,
            "annotations": annotations,
            "open_count": annotations.iter().filter(|a| a.get("status").and_then(|v| v.as_str()) == Some("open")).count(),
        })
    }

    /// Get the metadata for a state-doc.
    ///
    /// Returns the doc name, description, last modified time, and summary stats.
    pub fn get_doc_metadata(&self, doc_name: &str) -> serde_json::Value {
        let metadata = self.get_metadata(doc_name);

        json!({
            "doc": doc_name,
            "metadata": metadata,
        })
    }

    /// Get all sections for a state-doc.
    ///
    /// Returns the full section tree with levels, titles, and line ranges.
    pub fn get_all_sections(&self, doc_name: &str) -> serde_json::Value {
        let sections = self.get_all_sections_for_doc(doc_name);

        json!({
            "doc": doc_name,
            "sections": sections,
        })
    }

    // --- Internal query methods ---

    fn get_section(&self, doc_name: &str, section_name: &str) -> Option<Section> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, level, title, start_line, end_line, parent_id, content_hash
              FROM sections
              WHERE doc_id = ?1 AND title = ?2
              ORDER BY level DESC LIMIT 1",
            )
            .ok()?;

        stmt.query_row(rusqlite::params![doc_name, section_name], |row| {
                Ok(Section {
                    id: row.get(0)?,
                    doc_name: String::new(),
                    level: row.get(1)?,
                    title: row.get(2)?,
                    start_line: row.get(3)?,
                    end_line: row.get(4)?,
                    parent_id: row.get(5)?,
                    child_count: 0,
                    content_hash: row.get(6)?,
                    is_confidence_section: false,
                })
            })
            .ok()
    }

    fn get_child_sections(&self, doc_name: &str, section_name: &str) -> Vec<serde_json::Value> {
        let pattern = format!("{}%", section_name);

        let Ok(mut stmt) = self.conn.prepare(
            "SELECT level, title, start_line, end_line
              FROM sections
              WHERE doc_id = ?1 AND title LIKE ?2
              ORDER BY level DESC",
        ) else {
            return Vec::new();
        };

        stmt.query_map(rusqlite::params![doc_name, pattern], |row| {
                Ok(json!({
                    "level": row.get::<_, i32>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "start_line": row.get::<_, i32>(2)?,
                    "end_line": row.get::<_, i32>(3)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    fn get_tables_for_section(&self, doc_name: &str, section_name: &str) -> Vec<serde_json::Value> {
        let Ok(mut stmt) = self.conn.prepare(
            "SELECT id, headers, rows
              FROM tables
              WHERE doc_id = ?1 AND section_id = (
                  SELECT id FROM sections WHERE doc_id = ?1 AND title = ?2
              )",
        ) else {
            return Vec::new();
        };

        stmt.query_map(rusqlite::params![doc_name, section_name], |row| {
                Ok(json!({
                    "index": row.get::<_, i32>(0)?,
                    "headers": row.get::<_, String>(1)?,
                    "rows": row.get::<_, String>(2)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    fn get_code_blocks_for_section(
        &self,
        doc_name: &str,
        section_name: &str,
    ) -> Vec<serde_json::Value> {
        let Ok(mut stmt) = self.conn.prepare(
            "SELECT id, language, content
              FROM code_blocks
              WHERE doc_id = ?1 AND section_id = (
                  SELECT id FROM sections WHERE doc_id = ?1 AND title = ?2
              )",
        ) else {
            return Vec::new();
        };

        stmt.query_map(rusqlite::params![doc_name, section_name], |row| {
                Ok(json!({
                    "index": row.get::<_, i32>(0)?,
                    "language": row.get::<_, String>(1)?,
                    "content": row.get::<_, String>(2)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    fn get_annotations_for_section(
        &self,
        doc_name: &str,
        section_name: &str,
    ) -> Vec<serde_json::Value> {
        let Ok(mut stmt) = self.conn.prepare(
            "SELECT line, kind, message, author, status
              FROM annotations
              WHERE doc_id = ?1 AND section_id = (
                  SELECT id FROM sections WHERE doc_id = ?1 AND title = ?2
              )",
        ) else {
            return Vec::new();
        };

        stmt.query_map(rusqlite::params![doc_name, section_name], |row| {
                Ok(json!({
                    "line": row.get::<_, i32>(0)?,
                    "kind": row.get::<_, String>(1)?,
                    "message": row.get::<_, String>(2)?,
                    "author": row.get::<_, String>(3)?,
                    "status": row.get::<_, String>(4)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    fn get_annotations_for_doc(&self, doc_name: &str) -> Vec<serde_json::Value> {
        let Ok(mut stmt) = self.conn.prepare(
            "SELECT line, kind, message, author, status
              FROM annotations
              WHERE doc_id = ?1",
        ) else {
            return Vec::new();
        };

        stmt.query_map(rusqlite::params![doc_name], |row| {
                Ok(json!({
                    "line": row.get::<_, i32>(0)?,
                    "kind": row.get::<_, String>(1)?,
                    "message": row.get::<_, String>(2)?,
                    "author": row.get::<_, String>(3)?,
                    "status": row.get::<_, String>(4)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    fn search_sections(&self, doc_name: &str, keyword: &str) -> Vec<serde_json::Value> {
        let pattern = format!("%{}%", keyword);

        let Ok(mut stmt) = self.conn.prepare(
            "SELECT id, level, title, start_line, end_line, content_hash
              FROM sections
              WHERE doc_id = ?1 AND title LIKE ?2
              ORDER BY level DESC",
        ) else {
            return Vec::new();
        };

        stmt.query_map(rusqlite::params![doc_name, pattern], |row| {
                Ok(json!({
                    "section_id": row.get::<_, i32>(0)?,
                    "level": row.get::<_, i32>(1)?,
                    "title": row.get::<_, String>(2)?,
                    "start_line": row.get::<_, i32>(3)?,
                    "end_line": row.get::<_, i32>(4)?,
                    "content_hash": row.get::<_, String>(5)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    fn search_docs_by_tags(&self, tags: &[&str]) -> Vec<serde_json::Value> {
        let mut results = Vec::new();

        for tag in tags {
            let rows: Vec<serde_json::Value> = {
                let Ok(mut stmt) = self.conn.prepare(
                    "SELECT doc_name, description, last_modified
                      FROM doc_metadata
                      WHERE doc_name LIKE ?1 OR description LIKE ?1",
                ) else {
                    continue;
                };

                stmt.query_map(rusqlite::params![format!("%{}%", tag)], |row| {
                    Ok(json!({
                        "name": row.get::<_, String>(0)?,
                        "description": row.get::<_, String>(1)?,
                        "last_modified": row.get::<_, String>(2)?,
                    }))
                })
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
            };

            for row in rows {
                if !results.iter().any(|r: &serde_json::Value| r["name"].as_str() == row["name"].as_str()) {
                    results.push(row);
                }
            }
        }

        results
    }

    fn get_confidence_assessment(&self, doc_name: &str) -> Option<serde_json::Value> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT what_captured, what_missed, assumptions, blind_spots, stale_after
              FROM confidence
              WHERE doc_id = ?1",
            )
            .ok()?;

        stmt.query_row(rusqlite::params![doc_name], |row| {
                Ok(json!({
                    "what_captured": row.get::<_, String>(0)?,
                    "what_missed": row.get::<_, String>(1)?,
                    "assumptions": row.get::<_, String>(2)?,
                    "blind_spots": row.get::<_, String>(3)?,
                    "stale_after": row.get::<_, String>(4)?,
                }))
            })
            .ok()
    }

    fn get_metadata(&self, doc_name: &str) -> Option<serde_json::Value> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT doc_name, description, last_modified, line_count, checksum
              FROM doc_metadata
              WHERE doc_name = ?1",
            )
            .ok()?;

        stmt.query_row(rusqlite::params![doc_name], |row| {
                Ok(json!({
                    "doc_name": row.get::<_, String>(0)?,
                    "description": row.get::<_, String>(1)?,
                    "last_modified": row.get::<_, String>(2)?,
                    "line_count": row.get::<_, i32>(3)?,
                    "checksum": row.get::<_, String>(4)?,
                }))
            })
            .ok()
    }

    fn get_all_sections_for_doc(&self, doc_name: &str) -> Vec<serde_json::Value> {
        let Ok(mut stmt) = self.conn.prepare(
            "SELECT id, level, title, start_line, end_line
              FROM sections
              WHERE doc_id = ?1
              ORDER BY start_line ASC",
        ) else {
            return Vec::new();
        };

        stmt.query_map(rusqlite::params![doc_name], |row| {
                Ok(json!({
                    "section_id": row.get::<_, i32>(0)?,
                    "level": row.get::<_, i32>(1)?,
                    "title": row.get::<_, String>(2)?,
                    "start_line": row.get::<_, i32>(3)?,
                    "end_line": row.get::<_, i32>(4)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }
}
