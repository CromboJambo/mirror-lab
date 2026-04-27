//! Diff module for computing change summaries between file states.
//!
//! This module implements the MVP diff strategy:
//! - Baseline: text-based line diffs for all files
//! - Enhanced: semantic diffs for JSON/TOML where reliable
//! - Fallback: Unknown format preserves raw snapshots
//!
//! The system never fails to record a change even if parsing is unavailable.

use crate::domain::{DiffFormat, DiffSummary};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error, warn};

/// Represents the result of computing a diff between two file states.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    /// The format used to compute this diff.
    pub format: DiffFormat,
    /// Summary statistics about the change.
    pub summary: DiffSummary,
    /// Key-level changes if semantic parsing succeeded.
    pub keys_changed: Vec<String>,
    /// Raw diff output for text-based diffs.
    pub raw_diff: Option<String>,
}

/// Computes a diff between old and new content.
pub struct DiffComputer;

impl DiffComputer {
    /// Default debounce duration for file events.
    pub const DEFAULT_DEBOUNCE_MS: u64 = 500;

    /// Computes a diff using the appropriate strategy based on format.
    pub fn compute(
        old_content: Option<&str>,
        new_content: &str,
        file_type: &crate::domain::FileType,
    ) -> Result<DiffResult> {
        debug!("Computing diff for {:?} format", file_type);

        match file_type {
            crate::domain::FileType::Json => Self::compute_json_diff(old_content, new_content),
            crate::domain::FileType::Toml => Self::compute_toml_diff(old_content, new_content),
            _ => Self::compute_text_diff(old_content, new_content),
        }
    }

    /// Computes a text-based line diff (baseline for all formats).
    pub fn compute_text_diff(old_content: Option<&str>, new_content: &str) -> Result<DiffResult> {
        let old_lines: Vec<&str> = old_content.map(|c| c.lines().collect()).unwrap_or_default();
        let new_lines: Vec<&str> = new_content.lines().collect();

        // Simple line-based comparison
        let mut lines_added = 0usize;
        let mut lines_removed = 0usize;
        let mut keys_changed = vec![];

        for (i, old_line) in old_lines.iter().enumerate() {
            if new_lines.iter().any(|nl| *nl == *old_line) {
                // Line exists in both - unchanged
                continue;
            } else if i < new_lines.len() && new_lines[i] != *old_line {
                // Modified line
                lines_removed += 1;
                lines_added += 1;
                if let Some(key_part) = old_line.split('=').next() {
                    keys_changed.push(format!("line_{}: {}", i, key_part));
                }
            } else {
                // Removed line
                lines_removed += 1;
                if let Some(key_part) = old_line.split('=').next() {
                    keys_changed.push(format!("removed: {}", key_part));
                }
            }
        }

        for (i, new_line) in new_lines.iter().enumerate() {
            if !old_lines.iter().any(|ol| *ol == *new_line) {
                lines_added += 1;
                if let Some(key_part) = new_line.split('=').next() {
                    keys_changed.push(format!("added: {}", key_part));
                }
            } else if i < old_lines.len() && old_lines[i] != *new_line {
                // Already counted as modified
            }
        }

        let has_changes = lines_added > 0 || lines_removed > 0;

        let raw_diff = if has_changes {
            Some(Self::format_text_diff(&old_lines, &new_lines))
        } else {
            None
        };

        let summary = DiffSummary {
            total_changes: lines_added + lines_removed,
            lines_added,
            lines_removed,
            is_material: lines_added > 0 || lines_removed > 0,
            keys_changed,
        };

        Ok(DiffResult {
            format: DiffFormat::Text,
            summary,
            keys_changed,
            raw_diff,
        })
    }

    /// Formats a simple text diff for display.
    fn format_text_diff(old_lines: &[&str], new_lines: &[&str]) -> String {
        use std::fmt::Write;

        let mut output = String::new();

        // Simple unified-style diff (not full git diff, just summary)
        write!(&mut output, "--- old\n+++ new\n").unwrap();

        for (i, old_line) in old_lines.iter().enumerate() {
            if !new_lines.iter().any(|nl| *nl == *old_line) {
                writeln!(&mut output, "-{}", old_line).unwrap();
            } else if i < new_lines.len() && old_lines[i] != new_lines[i] {
                writeln!(&mut output, "-{}", old_line).unwrap();
                writeln!(&mut output, "+{}", new_lines[i]).unwrap();
            }
        }

        for (i, new_line) in new_lines.iter().enumerate() {
            if !old_lines.iter().any(|ol| *ol == *new_line) && i >= old_lines.len() {
                writeln!(&mut output, "+{}", new_line).unwrap();
            } else if i < old_lines.len() && old_lines[i] != *new_line {
                // Already shown as modified
            }
        }

        output
    }

    /// Computes a semantic diff for JSON files.
    pub fn compute_json_diff(old_content: Option<&str>, new_content: &str) -> Result<DiffResult> {
        use serde_json::Value;

        let old_obj: Option<Value> = old_content.and_then(|c| match serde_json::from_str(c) {
            Ok(v) => Some(v),
            Err(e) => {
                warn!("Failed to parse JSON for diff: {}", e);
                None
            }
        });

        let new_obj: Value = match serde_json::from_str(new_content) {
            Ok(v) => v,
            Err(e) => {
                // Fall back to text diff if parsing fails
                error!("JSON parse failed, falling back to text diff: {}", e);
                return Self::compute_text_diff(old_content, new_content);
            }
        };

        let mut keys_changed = vec![];
        let (added, removed, updated) =
            Self::compare_json_values(&old_obj, Some(&new_obj), &mut keys_changed);

        let summary = DiffSummary {
            total_changes: added + removed + updated,
            lines_added: added,
            lines_removed: removed,
            is_material: added > 0 || removed > 0 || updated > 0,
            keys_changed,
        };

        Ok(DiffResult {
            format: DiffFormat::Json,
            summary,
            keys_changed,
            raw_diff: None, // Semantic diff doesn't produce text diff
        })
    }

    /// Recursively compares JSON values and tracks changes.
    fn compare_json_values(
        old_val: &Option<Value>,
        new_val: Option<&Value>,
        keys_changed: &mut Vec<String>,
    ) -> (usize, usize, usize) {
        let mut added = 0;
        let mut removed = 0;
        let mut updated = 0;

        if let Some(old_obj) = old_val {
            if let (Value::Object(old_map), Some(Value::Object(new_map))) = (old_obj, new_val) {
                // Check for removed keys
                for (key, old_value) in old_map {
                    match new_map.get(key) {
                        None => {
                            removed += 1;
                            keys_changed.push(format!("removed: {}", key));
                        }
                        Some(new_value) => {
                            if old_value != new_value {
                                updated += 1;
                                keys_changed.push(format!(
                                    "updated: {} ({:?} -> {:?})",
                                    key, old_value, new_value
                                ));
                            }
                        }
                    }
                }

                // Check for added keys
                for (key, value) in new_map {
                    if !old_map.contains_key(key) {
                        added += 1;
                        keys_changed.push(format!("added: {} ({:?})", key, value));
                    } else {
                        // Already compared as updated or unchanged
                        continue;
                    }
                }
            } else if old_val != &new_val.unwrap_or(&Value::Null) {
                // Simple values changed
                updated += 1;
                keys_changed.push(format!("root value: {:?} -> {:?}", old_val, new_val));
            }
        } else if let Some(new_value) = new_val {
            // New content appeared
            added += 1;
            keys_changed.push(format!("added (new root): {:?}", new_value));
        }

        (added, removed, updated)
    }

    /// Computes a semantic diff for TOML files.
    pub fn compute_toml_diff(old_content: Option<&str>, new_content: &str) -> Result<DiffResult> {
        use toml::Value;

        let old_table: Option<Value> = old_content.and_then(|c| match c.parse::<Value>() {
            Ok(v) => Some(v),
            Err(e) => {
                warn!("Failed to parse TOML for diff: {}", e);
                None
            }
        });

        let new_table: Value = match new_content.parse::<Value>() {
            Ok(v) => v,
            Err(e) => {
                error!("TOML parse failed, falling back to text diff: {}", e);
                return Self::compute_text_diff(old_content, new_content);
            }
        };

        let mut keys_changed = vec![];
        let (added, removed, updated) =
            Self::compare_toml_values(&old_table, Some(&new_table), &mut keys_changed);

        let summary = DiffSummary {
            total_changes: added + removed + updated,
            lines_added: added,
            lines_removed: removed,
            is_material: added > 0 || removed > 0 || updated > 0,
            keys_changed,
        };

        Ok(DiffResult {
            format: DiffFormat::Toml,
            summary,
            keys_changed,
            raw_diff: None,
        })
    }

    /// Recursively compares TOML values and tracks changes.
    fn compare_toml_values(
        old_val: &Option<&Value>,
        new_val: Option<&Value>,
        keys_changed: &mut Vec<String>,
    ) -> (usize, usize, usize) {
        let mut added = 0;
        let mut removed = 0;
        let mut updated = 0;

        if let (Some(&Value::Table(old_table)), Some(&Value::Table(new_table))) = (old_val, new_val)
        {
            // Check for removed keys
            for (key, old_value) in old_table.iter() {
                match new_table.get(key) {
                    None => {
                        removed += 1;
                        keys_changed.push(format!("removed: {}", key));
                    }
                    Some(new_value) => {
                        if old_value != new_value {
                            updated += 1;
                            keys_changed.push(format!(
                                "updated: {} ({:?} -> {:?})",
                                key, old_value, new_value
                            ));
                        }
                    }
                }
            }

            // Check for added keys
            for (key, value) in new_table.iter() {
                if !old_table.contains_key(key) {
                    added += 1;
                    keys_changed.push(format!("added: {} ({:?})", key, value));
                }
            }
        } else if old_val != &new_val {
            // Simple values changed
            updated += 1;
            keys_changed.push(format!(
                "root value: {:?} -> {:?}",
                old_val.map(|v| v.to_string()),
                new_val.map(|v| v.to_string())
            ));
        }

        (added, removed, updated)
    }

    /// Computes a summary without semantic parsing (for Lua, KDL, etc.).
    pub fn compute_fallback_summary(old_content: Option<&str>, new_content: &str) -> DiffSummary {
        let old_lines = old_content.map(|c| c.lines().count()).unwrap_or(0);
        let new_lines = new_content.lines().count();

        let lines_added = (new_lines as i32 - old_lines as i32).max(0) as usize;
        let lines_removed = (old_lines as i32 - new_lines as i32).max(0) as usize;

        DiffSummary {
            total_changes: lines_added + lines_removed,
            lines_added,
            lines_removed,
            is_material: old_content.map_or(true, |c| c != new_content),
            keys_changed: vec![], // Unknown format - no semantic info
        }
    }

    /// Determines the appropriate diff format for a file type.
    pub fn determine_format(file_type: &crate::domain::FileType) -> DiffFormat {
        match file_type {
            crate::domain::FileType::Json => DiffFormat::Json,
            crate::domain::FileType::Toml => DiffFormat::Toml,
            _ => DiffFormat::Text,
        }
    }

    /// Creates a default summary when no diff can be computed.
    pub fn create_default_summary() -> DiffSummary {
        DiffSummary {
            total_changes: 0,
            lines_added: 0,
            lines_removed: 0,
            is_material: false,
            keys_changed: vec![],
        }
    }

    /// Serializes a diff summary to JSON for storage.
    pub fn serialize_summary(summary: &DiffSummary) -> String {
        serde_json::to_string(summary).unwrap_or_else(|_| "{}".to_string())
    }

    /// Deserializes a diff summary from JSON.
    pub fn deserialize_summary(json: &str) -> Result<DiffSummary> {
        serde_json::from_str(json).context("Failed to parse diff summary")
    }
}

/// Adapter trait for custom diff strategies (extensibility point).
pub trait DiffAdapter: Send + Sync {
    /// Returns the file types this adapter handles.
    fn handles(&self) -> Vec<crate::domain::FileType>;

    /// Attempts to compute a semantic diff using this strategy.
    fn try_compute(
        &self,
        old_content: Option<&str>,
        new_content: &str,
    ) -> Result<Option<DiffResult>>;
}

/// Default adapter factory that creates appropriate adapters for file types.
pub struct AdapterFactory;

impl AdapterFactory {
    /// Creates a list of default adapters in priority order.
    pub fn create_default_adapters() -> Vec<Box<dyn DiffAdapter>> {
        // JSON and TOML have built-in support via compute_json_diff/compute_toml_diff
        // Custom adapters can be added here for future extensions
        vec![]
    }

    /// Finds an adapter that handles a given file type.
    pub fn find_adapter(
        adapters: &[Box<dyn DiffAdapter>],
        file_type: &crate::domain::FileType,
    ) -> Option<&dyn DiffAdapter> {
        adapters.iter().find(|a| a.handles().contains(file_type))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::FileType;

    #[test]
    fn test_text_diff_no_changes() {
        let result = DiffComputer::compute_text_diff(Some("line1\nline2"), "line1\nline2").unwrap();
        assert_eq!(result.summary.lines_added, 0);
        assert_eq!(result.summary.lines_removed, 0);
        assert!(!result.summary.is_material);
    }

    #[test]
    fn test_text_diff_with_additions() {
        let result = DiffComputer::compute_text_diff(Some("line1"), "line1\nline2").unwrap();
        assert_eq!(result.summary.lines_added, 1);
        assert_eq!(result.summary.lines_removed, 0);
        assert!(result.summary.is_material);
    }

    #[test]
    fn test_text_diff_with_removals() {
        let result = DiffComputer::compute_text_diff(Some("line1\nline2"), "line1").unwrap();
        assert_eq!(result.summary.lines_added, 0);
        assert_eq!(result.summary.lines_removed, 1);
        assert!(result.summary.is_material);
    }

    #[test]
    fn test_json_diff_adds_key() {
        let old = r#"{"theme": "dark", "font_size": 12}"#;
        let new = r#"{"theme": "dark", "font_size": 12, "editor": "zed"}"#;

        let result = DiffComputer::compute_json_diff(Some(old), new).unwrap();
        assert_eq!(result.format, DiffFormat::Json);
        assert_eq!(result.summary.keys_added, 1); // keys_added is derived from added in summary
        assert!(result.summary.is_material);
    }

    #[test]
    fn test_json_diff_updates_key() {
        let old = r#"{"font_size": 12}"#;
        let new = r#"{"font_size": 14}"#;

        let result = DiffComputer::compute_json_diff(Some(old), new).unwrap();
        assert_eq!(result.format, DiffFormat::Json);
        assert!(result.summary.is_material);
    }

    #[test]
    fn test_toml_diff_adds_key() {
        let old = r#"theme = "dark"
font_size = 12"#;
        let new = r#"theme = "dark"
font_size = 12
editor = "zed""#;

        let result = DiffComputer::compute_toml_diff(Some(old), new).unwrap();
        assert_eq!(result.format, DiffFormat::Toml);
        assert!(result.summary.is_material);
    }

    #[test]
    fn test_json_parse_fallback_to_text() {
        // Invalid JSON should fall back to text diff
        let old = r#"valid json"#;
        let new = r#"also valid"#;

        let result = DiffComputer::compute_json_diff(Some(old), new).unwrap();
        assert_eq!(result.format, DiffFormat::Text); // Should fallback
    }

    #[test]
    fn test_determine_format() {
        assert_eq!(
            DiffComputer::determine_format(&FileType::Json),
            DiffFormat::Json
        );
        assert_eq!(
            DiffComputer::determine_format(&FileType::Toml),
            DiffFormat::Toml
        );
        assert_eq!(
            DiffComputer::determine_format(&FileType::Lua),
            DiffFormat::Text
        );
    }

    #[test]
    fn test_fallback_summary() {
        let summary = DiffComputer::compute_fallback_summary(Some("1\n2\n3"), "1\n2");
        assert_eq!(summary.lines_added, 0);
        assert_eq!(summary.lines_removed, 1);
        assert!(summary.is_material);
    }

    #[test]
    fn test_serialize_deserialize_summary() {
        let summary = DiffSummary {
            total_changes: 5,
            lines_added: 3,
            lines_removed: 2,
            is_material: true,
            keys_changed: vec!["key1".to_string()],
        };

        let json = DiffComputer::serialize_summary(&summary);
        let parsed: DiffSummary = DiffComputer::deserialize_summary(&json).unwrap();

        assert_eq!(parsed.total_changes, summary.total_changes);
    }
}
