// crabjar/memory/src/state_docs/indexer.rs
// Markdown indexer that parses state-docs into SQLite rows

use crate::state_docs::models::{CodeBlock, ConfidenceAssessment, DocMetadata, Section, Table};
use chrono::Utc;

use rusqlite::{Connection, params};
use std::fs;
use std::path::Path;

/// Index a single state-doc Markdown file into SQLite
pub fn index_doc(
    conn: &Connection,
    doc_path: &Path,
    overlay_entries: &[crate::state_docs::models::Annotation],
) -> Result<(), crate::Error> {
    let content = fs::read_to_string(doc_path)?;
    let metadata = extract_metadata(&content);
    let sections = extract_sections(&content);
    let tables = extract_tables(&content);
    let code_blocks = extract_code_blocks(&content);
    let confidence = extract_confidence(&content);

    // Insert doc metadata
    insert_doc_metadata(conn, doc_path, &metadata)?;

    // Insert sections
    for section in &sections {
        insert_section(conn, doc_path, section)?;
    }

    // Insert tables
    for table in &tables {
        insert_table(conn, doc_path, table)?;
    }

    // Insert code blocks
    for block in &code_blocks {
        insert_code_block(conn, doc_path, block)?;
    }

    // Insert confidence assessment
    if let Some(conf) = confidence {
        insert_confidence(conn, doc_path, &conf)?;
    }

    // Insert annotations linked by line
    for annotation in overlay_entries {
        insert_annotation(conn, doc_path, annotation)?;
    }

    Ok(())
}

/// Index all state-docs in a directory into SQLite
pub fn index_all_docs(conn: &Connection, docs_dir: &Path) -> Result<usize, crate::Error> {
    if !docs_dir.exists() {
        return Ok(0);
    }

    let mut count = 0;
    for entry in fs::read_dir(docs_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");

        if file_name == "README.md" {
            continue;
        }

        // Load overlay annotations for this doc
        let overlay_path = docs_dir
            .parent()
            .unwrap_or(docs_dir)
            .join("overlay")
            .join(format!(
                "{}.overlay.json",
                file_name.trim_end_matches(".md")
            ));

        let overlay_entries = if overlay_path.exists() {
            load_overlay(&overlay_path)?
        } else {
            Vec::new()
        };

        index_doc(conn, &path, &overlay_entries)?;
        count += 1;
    }

    Ok(count)
}

/// Extract metadata from the frontmatter of a Markdown file
fn extract_metadata(content: &str) -> DocMetadata {
    let mut doc_name = String::new();
    let mut description = String::new();

    // Parse YAML frontmatter (--- delimited)
    if content.starts_with("---") {
        let parts: Vec<&str> = content.split("---").collect();
        if parts.len() >= 3 {
            let frontmatter = parts[1];
            for line in frontmatter.lines() {
                if let Some(key_val) = line.split_once(':') {
                    let key = key_val.0.trim();
                    let val = key_val.1.trim();
                    match key {
                        "name" => doc_name = val.to_string(),
                        "description" => description = val.to_string(),
                        _ => {}
                    }
                }
            }
        }
    }

    // Extract display_name from first h1
    let display_name = content
        .lines()
        .find(|line| line.starts_with("# "))
        .map(|line| line.trim_start_matches("# ").trim())
        .unwrap_or("Untitled");

    DocMetadata {
        doc_name,
        display_name: display_name.to_string(),
        description,
        path: doc_path_to_string(content),
        last_modified: Utc::now(),
        line_count: content.lines().count(),
        section_count: 0,
        table_count: 0,
        code_block_count: 0,
        annotation_count: 0,
        open_annotation_count: 0,
        checksum: compute_checksum(content),
    }
}

fn compute_checksum(content: &str) -> String {
    let mut hash = 0u64;
    for byte in content.bytes() {
        hash = hash.wrapping_add(byte as u64).wrapping_mul(31);
    }
    format!("{:x}", hash)
}

fn doc_path_to_string(_content: &str) -> String {
    "unknown".to_string()
}

/// Extract sections (h1, h2, h3) with line ranges
fn extract_sections(content: &str) -> Vec<Section> {
    let lines: Vec<&str> = content.lines().collect();
    let mut sections = Vec::new();
    let mut current_section: Option<Section> = None;
    let mut section_id_counter = 1i64;

    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1; // 1-indexed

        if line.starts_with("# ") {
            // Start new h1 section
            if let Some(mut s) = current_section.take() {
                s.end_line = line_num - 1;
                sections.push(s);
            }

            // Start new h2 section
            if let Some(mut s) = current_section.take() {
                s.end_line = line_num - 1;
                sections.push(s);
            }

            // Start new h3 section
            if let Some(mut s) = current_section.take() {
                s.end_line = line_num - 1;
                sections.push(s);
            }
            let title = line.trim_start_matches("### ").trim();
            current_section = Some(Section {
                id: section_id_counter,
                doc_name: String::new(),
                level: 3,
                title: title.to_string(),
                start_line: line_num,
                end_line: 0,
                parent_id: current_section.as_ref().map(|s| s.id),
                child_count: 0,
                content_hash: String::new(),
                is_confidence_section: false,
            });
            section_id_counter += 1;
        }

        // Accumulate content hash for current section
        if let Some(ref mut s) = current_section {
            let mut h: u64 = 0;
            for byte in line.bytes() {
                h = h.wrapping_add(byte as u64).wrapping_mul(31);
            }
            s.content_hash = format!("{:x}", h);
        }
    }

    // Close the last section
    if let Some(mut s) = current_section.take() {
        s.end_line = lines.len();
        sections.push(s);
    }

    sections
}

/// Extract tables from Markdown content
fn extract_tables(content: &str) -> Vec<Table> {
    let lines: Vec<&str> = content.lines().collect();
    let mut tables = Vec::new();
    let mut in_table = false;
    let mut current_table: Option<Table> = None;
    let _row_count = 0;
    let mut table_id_counter = 1i64;

    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;

        // Detect table start (line with | characters and at least 2 columns)
        if !in_table && line.contains('|') && line.split('|').count() >= 3 {
            // Check if it's a separator line (---)
            if is_separator_line(line) {
                continue;
            }
            // Check if previous line is a header-like line
            if i > 0 {
                let prev_line = lines[i - 1];
                if prev_line.starts_with("# ") || prev_line.starts_with("## ") {
                    in_table = true;
                    current_table = Some(Table {
                        id: table_id_counter,
                        doc_name: String::new(),
                        section_id: 0,
                        start_line: line_num,
                        end_line: 0,
                        headers: extract_header_from_line(line),
                        row_count: 0,
                        content_hash: String::new(),
                    });
                    table_id_counter += 1;
                }
            }
        } else if in_table && line.contains('|') {
            if let Some(ref mut t) = current_table {
                let row = extract_row_from_line(line);
                t.row_count += 1;
                let mut h: u64 = 0;
                for byte in row.iter().flat_map(|s| s.bytes()) {
                    h = h.wrapping_add(byte as u64).wrapping_mul(31);
                }
                t.content_hash = format!("{:x}", h);
            }
        } else if in_table && !line.contains('|') {
            // End of table
            if let Some(mut t) = current_table.take() {
                t.end_line = line_num - 1;
                tables.push(t);
            }
            in_table = false;
        }
    }

    // Close any open table
    if in_table
        && let Some(mut t) = current_table.take() {
        t.end_line = lines.len();
        tables.push(t);
    }

    tables
}

/// Extract code blocks from Markdown content
fn extract_code_blocks(content: &str) -> Vec<CodeBlock> {
    let lines: Vec<&str> = content.lines().collect();
    let mut code_blocks = Vec::new();
    let mut in_block = false;
    let mut current_block: Option<CodeBlock> = None;
    let mut block_content = String::new();
    let mut block_id_counter = 1i64;

    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;

        if !in_block && line.starts_with("```") {
            in_block = true;
            let lang = line.trim_start_matches("```").trim();
            current_block = Some(CodeBlock {
                id: block_id_counter,
                doc_name: String::new(),
                section_id: 0,
                start_line: line_num,
                end_line: 0,
                language: lang.to_string(),
                content_hash: String::new(),
                line_count: 0,
            });
            block_id_counter += 1;
            block_content.clear();
        } else if in_block && line.starts_with("```") {
            // End of block
            if let Some(ref mut b) = current_block {
                b.end_line = line_num;
                b.line_count = block_content.lines().count();
                let mut h: u64 = 0;
                for byte in block_content.bytes() {
                    h = h.wrapping_add(byte as u64).wrapping_mul(31);
                }
                b.content_hash = format!("{:x}", h);
                code_blocks.push(b.clone());
            }
            in_block = false;
            current_block = None;
            block_content.clear();
        } else if in_block {
            block_content.push_str(line);
            block_content.push('\n');
        }
    }

    code_blocks
}

/// Extract confidence assessment from Markdown content
fn extract_confidence(content: &str) -> Option<ConfidenceAssessment> {
    let lines: Vec<&str> = content.lines().collect();
    let mut in_confidence = false;
    let mut confidence: Option<ConfidenceAssessment> = None;
    let mut current_key: Option<String> = None;
    let mut current_value_lines = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let _line_num = i + 1;

        // Detect confidence section (typically "## 8. Confidence Assessment")
        if line.starts_with("## 8. Confidence Assessment") {
            in_confidence = true;
            confidence = Some(ConfidenceAssessment {
                doc_name: String::new(),
                section_id: 0,
                what_captured: String::new(),
                what_missed: String::new(),
                assumptions: Vec::new(),
                blind_spots: Vec::new(),
                stale_after: String::new(),
                captured_at: Utc::now(),
            });
            continue;
        }

        if in_confidence {
            // Detect subsections
            if line.starts_with("### 8.1 What This Review Captures") {
                current_key = Some("what_captured".to_string());
                continue;
            }
            if line.starts_with("### 8.2 What This Review Might Have Missed") {
                current_key = Some("what_missed".to_string());
                continue;
            }
            if line.starts_with("### 8.3 Assumptions") {
                current_key = Some("assumptions".to_string());
                continue;
            }
            if line.starts_with("### 8.4 Blind Spots") {
                current_key = Some("blind_spots".to_string());
                continue;
            }
            if line.starts_with("### 8.5 Stale After") {
                current_key = Some("stale_after".to_string());
                continue;
            }

            // End of confidence section
            if line.starts_with("## ") && !line.starts_with("## 8.") {
                if let Some(ref mut c) = confidence {
                    match current_key.as_ref() {
                        Some(key) if key == "what_captured" => {
                            c.what_captured = current_value_lines.join("\n")
                        }
                        Some(key) if key == "what_missed" => {
                            c.what_missed = current_value_lines.join("\n")
                        }
                        Some(key) if key == "assumptions" => {
                            c.assumptions = current_value_lines.clone()
                        }
                        Some(key) if key == "blind_spots" => {
                            c.blind_spots = current_value_lines.clone()
                        }
                        Some(key) if key == "stale_after" => {
                            c.stale_after = current_value_lines.join("\n")
                        }
                        _ => {}
                    }
                }
                in_confidence = false;
                current_key = None;
                current_value_lines.clear();
                continue;
            }

            // Accumulate content for current key
            #[allow(clippy::collapsible_if)]
            if let Some(ref mut c) = confidence {
                if let Some(ref key) = current_key {
                    match key.as_str() {
                        "what_captured" => c.what_captured.push_str(line),
                        "what_missed" => c.what_missed.push_str(line),
                        "assumptions" => c.assumptions.push(line.trim().to_string()),
                        "blind_spots" => c.blind_spots.push(line.trim().to_string()),
                        "stale_after" => c.stale_after.push_str(line),
                        _ => {}
                    }
                    current_value_lines.push(line.to_string());
                }
            }
        }
    }

    // Close any open key
    #[allow(clippy::collapsible_if)]
    if in_confidence {
        if let Some(ref mut c) = confidence {
            match current_key.as_ref() {
                Some(key) if key == "what_captured" => {
                    c.what_captured = current_value_lines.join("\n")
                }
                Some(key) if key == "what_missed" => {
                    c.what_missed = current_value_lines.join("\n")
                }
                Some(key) if key == "assumptions" => {
                    c.assumptions = current_value_lines.clone()
                }
                Some(key) if key == "blind_spots" => {
                    c.blind_spots = current_value_lines.clone()
                }
                Some(key) if key == "stale_after" => {
                    c.stale_after = current_value_lines.join("\n")
                }
                _ => {}
            }
        }
    }

    confidence
}

/// Helper functions for table extraction.
fn is_separator_line(line: &str) -> bool {
    let parts: Vec<&str> = line.split('|').collect();
    parts.iter().all(|p| {
        let trimmed = p.trim();
        trimmed.is_empty() || trimmed.starts_with('-') || trimmed == "---"
    })
}

fn extract_header_from_line(line: &str) -> Vec<String> {
    line.split('|').map(|p| p.trim().to_string()).collect()
}

fn extract_row_from_line(line: &str) -> Vec<String> {
    line.split('|').map(|p| p.trim().to_string()).collect()
}

/// Load overlay annotations from JSON file
fn load_overlay(path: &Path) -> Result<Vec<crate::state_docs::models::Annotation>, crate::Error> {
    let content = fs::read_to_string(path)?;
    let overlay: Vec<crate::state_docs::models::Annotation> =
        serde_json::from_str(&content).map_err(crate::Error::Json)?;
    Ok(overlay)
}

/// Insert doc metadata into SQLite
fn insert_doc_metadata(
    conn: &Connection,
    doc_path: &Path,
    metadata: &DocMetadata,
) -> Result<(), crate::Error> {
    let file_name = doc_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    let checksum_json = serde_json::to_string(&metadata.checksum).map_err(crate::Error::Json)?;

    conn.execute(
        "INSERT OR REPLACE INTO doc_metadata (doc_name, description, last_modified, line_count, checksum)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            file_name,
            metadata.description,
            metadata.last_modified.to_rfc3339(),
            metadata.line_count,
            checksum_json,
        ],
    )?;

    Ok(())
}

/// Insert section into SQLite
fn insert_section(
    conn: &Connection,
    doc_path: &Path,
    section: &Section,
) -> Result<(), crate::Error> {
    let file_name = doc_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    conn.execute(
        "INSERT INTO sections (doc_id, level, title, start_line, end_line, parent_id, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            file_name,
            section.level,
            section.title,
            section.start_line,
            section.end_line,
            section.parent_id,
            section.content_hash,
        ],
    )?;

    Ok(())
}

/// Insert table into SQLite
fn insert_table(conn: &Connection, doc_path: &Path, table: &Table) -> Result<(), crate::Error> {
    let file_name = doc_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    let headers_json = serde_json::to_string(&table.headers).map_err(crate::Error::Json)?;

    conn.execute(
        "INSERT INTO tables (doc_id, section_id, start_line, end_line, headers)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            file_name,
            table.section_id,
            table.start_line,
            table.end_line,
            headers_json,
        ],
    )?;

    Ok(())
}

/// Insert code block into SQLite
fn insert_code_block(
    conn: &Connection,
    doc_path: &Path,
    block: &CodeBlock,
) -> Result<(), crate::Error> {
    let file_name = doc_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    conn.execute(
        "INSERT INTO code_blocks (doc_id, section_id, start_line, end_line, language, content, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            file_name,
            block.section_id,
            block.start_line,
            block.end_line,
            block.language,
            block.content_hash,
            block.content_hash,
        ],
    )?;

    Ok(())
}

/// Insert confidence assessment into SQLite
fn insert_confidence(
    conn: &Connection,
    doc_path: &Path,
    confidence: &ConfidenceAssessment,
) -> Result<(), crate::Error> {
    let file_name = doc_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    let assumptions_json =
        serde_json::to_string(&confidence.assumptions).map_err(crate::Error::Json)?;
    let blind_spots_json =
        serde_json::to_string(&confidence.blind_spots).map_err(crate::Error::Json)?;

    conn.execute(
        "INSERT INTO confidence (doc_id, what_captured, what_missed, assumptions, blind_spots, stale_after)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            file_name,
            confidence.what_captured,
            confidence.what_missed,
            assumptions_json,
            blind_spots_json,
            confidence.stale_after,
        ],
    )?;

    Ok(())
}

/// Insert annotation into SQLite
fn insert_annotation(
    conn: &Connection,
    doc_path: &Path,
    annotation: &crate::state_docs::models::Annotation,
) -> Result<(), crate::Error> {
    let file_name = doc_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    conn.execute(
        "INSERT INTO annotations (doc_id, section_id, line, kind, status, author, message, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            file_name,
            annotation.doc_name,
            annotation.section_id,
            annotation.line,
            annotation.kind,
            annotation.status,
            annotation.author,
            annotation.message,
            annotation.created_at.to_rfc3339(),
        ],
    )?;

    Ok(())
}
