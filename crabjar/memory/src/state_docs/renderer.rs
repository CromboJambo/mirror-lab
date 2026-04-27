use crate::state_docs::models::{Annotation, ConfidenceAssessment, DocMetadata, Section};
use chrono::Utc;

use rusqlite::{Connection, params};
use serde_json::json;

/// Renders a state-doc from indexed SQLite data with configurable zoom depth.
///
/// Zoom levels:
/// - 1: Overview — section titles + summaries (like a table of contents)
/// - 2: Section — full section content + annotations
/// - 3: Paragraph — paragraph-level detail + annotations + confidence assessment
pub struct Renderer<'a> {
    conn: &'a Connection,
}

impl<'a> Renderer<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Render a state-doc at the given zoom level.
    /// Returns (markdown_text, metadata_json).
    pub fn render_doc(
        &self,
        doc_name: &str,
        zoom: u8,
    ) -> Result<(String, serde_json::Value), rusqlite::Error> {
        let metadata = self.fetch_doc_metadata(doc_name)?;
        let sections = self.fetch_sections(doc_name)?;

        let markdown = match zoom {
            1 => self.render_overview(&sections, &metadata),
            2 => self.render_section_view(&sections, &metadata),
            3 => self.render_paragraph_view(&sections, &metadata),
            _ => self.render_section_view(&sections, &metadata),
        };

        let meta = json!({
            "doc": doc_name,
            "zoom": zoom,
            "sections_count": sections.len(),
            "last_modified": metadata.last_modified,
            "description": metadata.description,
        });

        Ok((markdown, meta))
    }

    /// Render a single section with annotations.
    pub fn render_section(
        &self,
        doc_name: &str,
        section_id: i64,
        zoom: u8,
    ) -> Result<(String, serde_json::Value), rusqlite::Error> {
        let section = self.fetch_section_by_id(section_id)?;
        let annotations = self.fetch_annotations_for_section(doc_name, section_id)?;

        let markdown = match zoom {
            1 => self.render_section_summary(&section, &annotations),
            2 => self.render_section_with_annotations(&section, &annotations),
            3 => self.render_section_detail(&section, &annotations),
            _ => self.render_section_with_annotations(&section, &annotations),
        };

        let meta = json!({
            "doc": doc_name,
            "section_id": section_id,
            "section_title": section.title,
            "section_level": section.level,
            "annotations_count": annotations.len(),
            "line_range": [section.start_line, section.end_line],
        });

        Ok((markdown, meta))
    }

    /// Render a paragraph-level view of a section.
    pub fn render_paragraph(
        &self,
        doc_name: &str,
        section_id: i64,
        paragraph_idx: usize,
        zoom: u8,
    ) -> Result<(String, serde_json::Value), rusqlite::Error> {
        let section = self.fetch_section_by_id(section_id)?;
        let annotations = self.fetch_annotations_for_section(doc_name, section_id)?;

        let markdown = match zoom {
            1 => self.render_paragraph_summary(&section, paragraph_idx, &annotations),
            2 => self.render_paragraph_detail(&section, paragraph_idx, &annotations),
            _ => self.render_paragraph_detail(&section, paragraph_idx, &annotations),
        };

        let meta = json!({
            "doc": doc_name,
            "section_id": section_id,
            "paragraph_idx": paragraph_idx,
            "line_range": self.get_paragraph_line_range(&section, paragraph_idx),
        });

        Ok((markdown, meta))
    }

    // ─── Fetch helpers ───

    fn fetch_doc_metadata(&self, doc_name: &str) -> Result<DocMetadata, rusqlite::Error> {
        self.conn.query_row(
            "SELECT description, last_modified FROM doc_metadata WHERE doc_name = ?1",
            params![doc_name],
            |row| {
                Ok(DocMetadata {
                    doc_name: doc_name.to_string(),
                    display_name: String::new(),
                    description: row.get::<_, String>(0)?,
                    path: String::new(),
                    last_modified: row.get::<_, String>(1).ok().and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))).unwrap_or_else(|| Utc::now()),
                    line_count: 0,
                    section_count: 0,
                    table_count: 0,
                    code_block_count: 0,
                    annotation_count: 0,
                    open_annotation_count: 0,
                    checksum: String::new(),
                })
            },
        )
    }

    fn fetch_sections(&self, doc_name: &str) -> Result<Vec<Section>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, level, title, start_line, end_line, parent_id, content_hash FROM sections WHERE doc_id = ?1 ORDER BY start_line ASC",
        )?;

        let rows = stmt.query_map(params![doc_name], |row| {
            Ok(Section {
                id: row.get(0)?,
                doc_name: String::new(),
                level: row.get(1)?,
                title: row.get(2)?,
                start_line: row.get(3)?,
                end_line: row.get(4)?,
                parent_id: None,
                child_count: 0,
                is_confidence_section: false,
                content_hash: row.get::<_, String>(6)?,
            })
        })?;

        let mut sections = Vec::new();
        for row in rows {
            sections.push(row?);
        }
        Ok(sections)
    }

    fn fetch_section_by_id(&self, section_id: i64) -> Result<Section, rusqlite::Error> {
        self.conn.query_row(
            "SELECT id, level, title, start_line, end_line, parent_id, content_hash FROM sections WHERE id = ?1",
            params![section_id],
            |row| {
                Ok(Section {
                    id: row.get(0)?,
                    doc_name: String::new(),
                    level: row.get(1)?,
                    is_confidence_section: false,
                    title: row.get(2)?,
                    start_line: row.get(3)?,
                    end_line: row.get(4)?,
                    parent_id: row.get(5)?,
                    child_count: 0,
                    content_hash: row.get::<_, String>(6)?,
                })
            },
        )
    }

    fn fetch_annotations_for_section(
        &self,
        doc_name: &str,
        section_id: i64,
    ) -> Result<Vec<Annotation>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, doc_id, section_id, line, kind, status, author, message, created_at FROM annotations WHERE doc_id = ?1 AND section_id = ?2 ORDER BY line ASC",
        )?;

        let rows = stmt.query_map(params![doc_name, section_id], |row| {
            Ok(Annotation {
                id: row.get(0)?,
                doc_name: String::new(),
                section_id: Some(row.get::<_, i64>(2)?),
                line: row.get(3)?,
                kind: row.get(4)?,
                status: row.get(5)?,
                author: row.get(6)?,
                message: row.get(7)?,
                created_at: row.get::<_, String>(8).ok().and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))).unwrap_or_else(|| Utc::now()),
            })
        })?;

        let mut annotations = Vec::new();
        for row in rows {
            annotations.push(row?);
        }
        Ok(annotations)
    }





    // ─── Render helpers ───

    fn render_overview(&self, sections: &[Section], metadata: &DocMetadata) -> String {
        let mut md = String::new();
        md.push_str(&format!("{}\n", metadata.description));
        md.push_str("\n---\n\n");

        // Group by level
        let level1: Vec<&Section> = sections.iter().filter(|s| s.level == 1).collect();
        let level2: Vec<&Section> = sections.iter().filter(|s| s.level == 2).collect();
        let level3: Vec<&Section> = sections.iter().filter(|s| s.level == 3).collect();

        if !level1.is_empty() {
            md.push_str("# # Sections Overview\n\n ");
            for s in level1 {
                md.push_str(&format!(
                    "# ## {} (lines {}-{})\n\n ",
                    s.title, s.start_line, s.end_line
                ));
            }
        }

        if !level2.is_empty() {
            md.push_str("# # Subsections\n\n ");
            for s in level2 {
                md.push_str(&format!(
                    "# ### {} (lines {}-{})\n\n ",
                    s.title, s.start_line, s.end_line
                ));
            }
        }

        if !level3.is_empty() {
            md.push_str("# # Paragraphs\n\n ");
            for s in level3 {
                md.push_str(&format!(
                    "# ## # {} (lines {}-{})\n\n ",
                    s.title, s.start_line, s.end_line
                ));
            }
        }

        // Add doubt block — every abstraction carries its own doubt
        md.push_str("# ## Doubt\n\n");
        md.push_str("### Assumptions Made\n\n");
        md.push_str("Overview groups sections by level. Intermediate headings between levels may be missing.\n\n");
        md.push_str("### Blind Spots\n\n");
        md.push_str("Content hash is stored in SQLite but content itself is not. This output shows headings only, not raw section content.\n\n");
        md.push_str("### Stale After\n\n");
        md.push_str(&format!("{} (last modified)\n\n", metadata.last_modified));

        md
    }

    fn render_section_view(&self, sections: &[Section], metadata: &DocMetadata) -> String {
        let mut md = String::new();
        md.push_str(&format!("{}\n", metadata.description));
        md.push_str("\n---\n\n");

        for section in sections {
            let heading_level = match section.level {
                1 => "#",
                2 => "# #",
                3 => "# ##",
                _ => "# ###",
            };

            md.push_str(&format!("{} {}\n\n", heading_level, section.title));
            md.push_str(&format!(
                "*Lines {}–{}\n\n",
                section.start_line, section.end_line
            ));

            // Show annotations
            let annotations = self
                .fetch_annotations_for_section("PLACEHOLDER", section.id)
                .unwrap_or_default();
            if !annotations.is_empty() {
                md.push_str("# ## Annotations\n\n ");
                for ann in annotations {
                    let status_marker = match ann.status.as_str() {
                        "open" => "🔵",
                        "resolved" => "✅",
                        _ => "⚪",
                    };
                    md.push_str(&format!(
                        "{} **{}** (line {}) by {}: {}\n\n",
                        status_marker, ann.kind, ann.line, ann.author, ann.message,
                    ));
                }
            }

            md.push_str("---\n\n");
        }

        // Add doubt block — every abstraction carries its own doubt
        md.push_str("# ## Doubt\n\n");
        md.push_str("### Assumptions Made\n\n");
        md.push_str("This reconstruction assumes sections are ordered by start_line. Missing intermediate headings may break the hierarchy.\n\n");
        md.push_str("### Blind Spots\n\n");
        md.push_str("Content hash is stored in SQLite but content itself is not. This output shows headings and annotations, not raw section content.\n\n");
        md.push_str("### Stale After\n\n");
        md.push_str(&format!("{} (last modified)\n\n", metadata.last_modified));

        md
    }

    fn render_paragraph_view(
        &self,
        sections: &[Section],
        metadata: &DocMetadata,
    ) -> String {
        let mut md = String::new();
        md.push_str(&format!("{}\n", metadata.description));
        md.push_str("\n---\n\n");

        for section in sections {
            let heading_level = match section.level {
                1 => "#",
                2 => "# #",
                3 => "# ##",
                _ => "# ###",
            };

            md.push_str(&format!("{} {}\n\n", heading_level, section.title));
            md.push_str(&format!(
                "*Lines {}–{}\n\n",
                section.start_line, section.end_line
            ));

            // Show annotations
            let annotations = self
                .fetch_annotations_for_section("PLACEHOLDER", section.id)
                .unwrap_or_default();
            if !annotations.is_empty() {
                md.push_str("# ## Annotations\n\n ");
                for ann in annotations {
                    let status_marker = match ann.status.as_str() {
                        "open" => "🔵",
                        "resolved" => "✅",
                        _ => "⚪",
                    };
                    md.push_str(&format!(
                        "{} **{}** (line {}) by {}: {}\n\n",
                        status_marker, ann.kind, ann.line, ann.author, ann.message,
                    ));
                }
            };

            md.push_str("---\n\n");
        };

        // Add doubt block — every abstraction carries its own doubt
        md.push_str("# ## Doubt\n\n");
        md.push_str("### Assumptions Made\n\n");
        md.push_str("This reconstruction assumes sections are ordered by start_line. Missing intermediate headings may break the hierarchy.\n\n");
        md.push_str("### Blind Spots\n\n");
        md.push_str("Content hash is stored in SQLite but content itself is not. This output shows headings and annotations, not raw section content.\n\n");
        md.push_str("### Stale After\n\n");
        md.push_str(&format!("{} (last modified)\n\n", metadata.last_modified));

        md
    }

    fn render_section_with_annotations(
        &self,
        section: &Section,
        annotations: &[Annotation],
    ) -> String {
        let mut md = String::new();
        let heading_level = match section.level {
            1 => "#",
            2 => "##",
            3 => "###",
            _ => "####",
        };
        md.push_str(&format!("{} {}\n\n", heading_level, section.title));
        md.push_str(&format!(
            "*Lines {}–{}\n\n",
            section.start_line, section.end_line
        ));

        if !annotations.is_empty() {
            md.push_str("# ## Annotations\n\n ");
            for ann in annotations {
                let status_marker = match ann.status.as_str() {
                    "open" => "🔵",
                    "resolved" => "✅",
                    _ => "⚪",
                };
                md.push_str(&format!(
                    "{} **{}** (line {}) by {}: {}\n\n",
                    status_marker, ann.kind, ann.line, ann.author, ann.message,
                ));
            }
        };

        md
    }

    fn render_section_detail(&self, section: &Section, annotations: &[Annotation]) -> String {
        let mut md = String::new();
        let heading_level = match section.level {
            1 => "#",
            2 => "##",
            3 => "###",
            _ => "####",
        };
        md.push_str(&format!("{} {}\n\n", heading_level, section.title));
        md.push_str(&format!(
            "*Lines {}–{}\n\n",
            section.start_line, section.end_line
        ));

        if !annotations.is_empty() {
            md.push_str("# ## Annotations (Detailed)\n\n ");
            for ann in annotations {
                md.push_str(&format!(
                    "- **{}** (line {}) by {}: {}\n",
                    ann.kind, ann.line, ann.author, ann.message,
                ));
                md.push_str(&format!("  - Status: {}\n", ann.status));
                md.push_str(&format!("  - Created: {}\n", ann.created_at));
                md.push_str(&format!("  - ID: {}\n", ann.id));
                md.push_str("\n");
            }
        };

        md
    }

    fn render_section_summary(
        &self,
        section: &Section,
        annotations: &[Annotation],
    ) -> String {
        let mut md = String::new();
        let heading_level = match section.level {
            1 => "#",
            2 => "##",
            3 => "###",
            _ => "####",
        };
        md.push_str(&format!(
            "{} {}\n\n",
            heading_level, section.title
        ));
        md.push_str(&format!("*Lines {}–{}\n\n", section.start_line, section.end_line));

        if !annotations.is_empty() {
            md.push_str("# ## Annotations\n\n ");
            for ann in annotations {
                md.push_str(&format!(
                    "- **{}** (line {}) by {}: {}\n",
                    ann.kind, ann.line, ann.author, ann.message,
                ));
            }
        };

        md
    }

    fn get_paragraph_line_range(&self, section: &Section, paragraph_idx: usize) -> [i64; 2] {
        let paragraph_count = estimate_paragraph_count(&format!("{}–{}", section.start_line, section.end_line)).max(1);
        let start = section.start_line + (paragraph_idx * (section.end_line - section.start_line) / paragraph_count);
        let end = start + (section.end_line - section.start_line) / paragraph_count;
        [start as i64, end as i64]
    }

    fn render_paragraph_summary(
        &self,
        section: &Section,
        paragraph_idx: usize,
        annotations: &[Annotation],
    ) -> String {
        let mut md = String::new();
        let heading_level = match section.level {
            1 => "#",
            2 => "##",
            3 => "###",
            _ => "####",
        };
        md.push_str(&format!(
            "{} {} — Paragraph {}\n\n",
            heading_level, section.title, paragraph_idx
        ));
        md.push_str(&format!("*Lines {}–{}\n\n", section.start_line, section.end_line));

        if !annotations.is_empty() {
            md.push_str("# ## Annotations\n\n ");
            for ann in annotations {
                md.push_str(&format!(
                    "- **{}** (line {}) by {}: {}\n",
                    ann.kind, ann.line, ann.author, ann.message,
                ));
            }
        };

        md
    }

    fn render_paragraph_detail(
        &self,
        section: &Section,
        paragraph_idx: usize,
        annotations: &[Annotation],
    ) -> String {
        let mut md = String::new();
        let heading_level = match section.level {
            1 => "#",
            2 => "##",
            3 => "###",
            _ => "####",
        };
        md.push_str(&format!(
            "{} {} — Paragraph {}\n\n",
            heading_level, section.title, paragraph_idx
        ));
        md.push_str(&format!("*Lines {}–{}\n\n", section.start_line, section.end_line));

        if !annotations.is_empty() {
            md.push_str("# ## Annotations\n\n ");
            for ann in annotations {
                let status_marker = match ann.status.as_str() {
                    "open" => "🔵",
                    "resolved" => "✅",
                    _ => "⚪",
                };
                md.push_str(&format!(
                    "{} **{}** (line {}) by {}: {}\n\n",
                    status_marker, ann.kind, ann.line, ann.author, ann.message,
                ));
            }
        };

        md
    }
}



fn truncate_snippet(snippet: &str, max_len: usize) -> String {
    if snippet.len() <= max_len {
        snippet.to_string()
    } else {
        format!("{}...", &snippet[..max_len])
    }
}

fn estimate_paragraph_count(snippet: &str) -> usize {
    // Count blank lines as paragraph separators
    snippet.split("\n\n").filter(|p| !p.is_empty()).count()
}

fn parse_confidence_from_snippet(snippet: &str) -> ConfidenceAssessment {
    let assessment = ConfidenceAssessment::default();

    for line in snippet.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("### What This Captures")
            || trimmed.starts_with("- What This Captures")
        {
            // Collect next lines until next heading
            continue;
        }
        if trimmed.starts_with("### What This Might Have Missed")
            || trimmed.starts_with("- What This Might Have Missed")
        {
            continue;
        }
        if trimmed.starts_with("### Assumptions") || trimmed.starts_with("- Assumptions") {
            continue;
        }
        if trimmed.starts_with("### Blind Spots") || trimmed.starts_with("- Blind Spots") {
            continue;
        }
        if trimmed.starts_with("### Stale After") || trimmed.starts_with("- Stale After") {
            continue;
        }
    }

    // Simplified parsing — in production this would use a proper Markdown parser
    assessment
}
