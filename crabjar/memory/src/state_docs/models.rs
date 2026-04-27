use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Metadata about a state-doc as a whole.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocMetadata {
    pub doc_name: String,
    pub display_name: String,
    pub description: String,
    pub path: String,
    pub last_modified: DateTime<Utc>,
    pub line_count: usize,
    pub section_count: usize,
    pub table_count: usize,
    pub code_block_count: usize,
    pub annotation_count: usize,
    pub open_annotation_count: usize,
    pub checksum: String,
}

/// A section in the state-doc hierarchy. 3 levels: h1 → h2 → h3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub id: i64,
    pub doc_name: String,
    pub level: u8, // 1, 2, or 3
    pub title: String,
    pub start_line: usize,
    pub end_line: usize,
    pub parent_id: Option<i64>,
    pub child_count: usize,
    pub content_hash: String,
    pub is_confidence_section: bool,
}

/// An extracted table from a state-doc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub id: i64,
    pub doc_name: String,
    pub section_id: i64,
    pub start_line: usize,
    pub end_line: usize,
    pub headers: Vec<String>,
    pub row_count: usize,
    pub content_hash: String,
}

/// A code block extracted from a state-doc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub id: i64,
    pub doc_name: String,
    pub section_id: i64,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub content_hash: String,
    pub line_count: usize,
}

/// The confidence assessment (doubt block) from a state-doc.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfidenceAssessment {
    pub doc_name: String,
    pub section_id: i64,
    pub what_captured: String,
    pub what_missed: String,
    pub assumptions: Vec<String>,
    pub blind_spots: Vec<String>,
    pub stale_after: String,
    pub captured_at: DateTime<Utc>,
}

/// An overlay annotation linked to a specific line in a state-doc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: i64,
    pub doc_name: String,
    pub section_id: Option<i64>,
    pub line: usize,
    pub kind: String, // "note" or "question"
    pub message: String,
    pub author: String,
    pub status: String, // "open" or "resolved"
    pub created_at: DateTime<Utc>,
}

/// A query result row that combines section content with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionRow {
    pub id: i64,
    pub doc_name: String,
    pub level: u8,
    pub title: String,
    pub start_line: usize,
    pub end_line: usize,
    pub parent_id: Option<i64>,
    pub child_count: usize,
    pub content_hash: String,
    pub is_confidence_section: bool,
    pub open_annotations: usize,
}

/// A table query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRow {
    pub id: i64,
    pub doc_name: String,
    pub section_id: i64,
    pub section_title: String,
    pub start_line: usize,
    pub end_line: usize,
    pub headers: Vec<String>,
    pub row_count: usize,
}

/// A code block query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlockRow {
    pub id: i64,
    pub doc_name: String,
    pub section_id: i64,
    pub section_title: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub line_count: usize,
}

/// A confidence assessment query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceRow {
    pub doc_name: String,
    pub section_id: i64,
    pub what_captured: String,
    pub what_missed: String,
    pub assumptions: Vec<String>,
    pub blind_spots: Vec<String>,
    pub stale_after: String,
    pub captured_at: DateTime<Utc>,
}

/// An annotation query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationRow {
    pub id: i64,
    pub doc_name: String,
    pub section_id: Option<i64>,
    pub section_title: Option<String>,
    pub line: usize,
    pub kind: String,
    pub message: String,
    pub author: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}
