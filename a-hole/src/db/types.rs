//! Database types module - defines entity structures for storage.
//!
//! These types mirror the domain types but are optimized for SQLite serialization.
//! They follow the MVP data model from kickstart.md exactly.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

/// Represents a file that is being watched for changes.
/// Maps to: `watched_files` table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchedFile {
    /// Unique identifier for this watched file entry.
    pub id: i64,
    /// The original path as provided by the user (may contain ~ or be relative).
    pub path: String,
    /// The normalized absolute path on disk.
    pub normalized_path: PathBuf,
    /// The tool associated with this config file (e.g., "wezterm", "nushell").
    pub tool: String,
    /// The detected file type based on extension.
    pub file_type: String,
    /// Current status of the watch target.
    pub status: String, // Active, Pending, Inactive
    /// When this file was first registered for watching.
    pub created_at: SystemTime,
    /// Last time a change was recorded for this file.
    pub updated_at: SystemTime,
}

/// A snapshot of file content at a specific point in time.
/// Maps to: `file_snapshots` table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    /// Unique identifier for this snapshot.
    pub id: i64,
    /// Reference to the watched file this snapshot belongs to.
    pub watched_file_id: i64,
    /// The full content of the file at capture time (stored inline).
    pub content: String,
    /// SHA-256 hash of the content for quick change detection.
    pub content_hash: String,
    /// When this snapshot was captured.
    pub captured_at: SystemTime,
}

/// A recorded change event when a watched file is modified.
/// Maps to: `config_changes` table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChange {
    /// Unique identifier for this change record.
    pub id: i64,
    /// Reference to the watched file that was changed.
    pub watched_file_id: i64,
    /// Reference to the snapshot before the change (if available).
    pub previous_snapshot_id: Option<i64>,
    /// Reference to the snapshot after the change (always present for updates).
    pub current_snapshot_id: i64,
    /// When the change was detected.
    pub timestamp: SystemTime,
    /// The type of change detected.
    pub change_kind: String, // updated, created, deleted
    /// Format used for the diff summary (text, json, toml).
    pub diff_format: String,
    /// JSON-encoded summary of the change.
    pub summary_json: String,
    /// JSON-encoded additional metadata about the change.
    pub metadata_json: String,
}

/// Metadata captured alongside a config change for extended tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeMetadata {
    /// Whether the file was readable at capture time.
    pub content_available: bool,
    /// How long the watcher took to detect the change (milliseconds).
    pub detection_latency_ms: Option<u64>,
    /// The editor or tool that made the change if known.
    pub editor_hint: Option<String>,
}

/// A revert operation record when a user reverses a prior change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertRecord {
    /// Unique identifier for this revert record.
    pub id: i64,
    /// The change ID being reverted (the target of the revert).
    pub target_change_id: i64,
    /// Reference to the watched file that was reverted.
    pub watched_file_id: i64,
    /// When the revert was performed.
    pub timestamp: SystemTime,
    /// Whether the revert succeeded or failed.
    pub success: bool,
    /// Error message if the revert failed.
    pub error_message: Option<String>,
}

/// Summary statistics about a diff computed from comparing states.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    /// Total number of lines/keys changed.
    pub total_changes: usize,
    /// Number of lines added.
    pub lines_added: usize,
    /// Number of lines removed.
    pub lines_removed: usize,
    /// Whether the change was material (not trivial/no-op).
    pub is_material: bool,
    /// Key-level changes if semantic parsing succeeded.
    pub keys_changed: Vec<String>,
}

/// Extension trait for converting domain types to DB types.
pub trait ToDbTypes {
    fn to_watched_file(&self) -> WatchedFile;
    fn to_snapshot(&self, watched_file_id: i64) -> FileSnapshot;
}

impl ToDbTypes for crate::domain::WatchedFile {
    fn to_watched_file(&self) -> WatchedFile {
        WatchedFile {
            id: self.id,
            path: self.path.clone(),
            normalized_path: self.normalized_path.clone(),
            tool: self.tool.clone(),
            file_type: self.file_type.as_str().to_string(),
            status: match self.status {
                crate::domain::WatchStatus::Active => "Active".to_string(),
                crate::domain::WatchStatus::Pending => "Pending".to_string(),
                crate::domain::WatchStatus::Inactive => "Inactive".to_string(),
            },
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    fn to_snapshot(&self, watched_file_id: i64) -> FileSnapshot {
        // This is a stub - actual snapshot creation happens in capture module
        FileSnapshot {
            id: 0,
            watched_file_id,
            content: String::new(),
            content_hash: String::new(),
            captured_at: self.updated_at,
        }
    }
}

/// Query result type for displaying config changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDisplay {
    pub id: i64,
    pub timestamp: SystemTime,
    pub tool: String,
    pub file_path: String,
    pub change_kind: String,
    pub diff_format: String,
    pub summary_json: String,
}

/// Query result type for listing watched files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchedFileDisplay {
    pub id: i64,
    pub path: String,
    pub normalized_path: PathBuf,
    pub tool: String,
    pub file_type: String,
    pub status: String,
}

/// Error types for database operations.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Database initialization failed: {0}")]
    InitFailed(String),

    #[error("Database query failed: {0}")]
    QueryFailed(String),

    #[error("Database write failed: {0}")]
    WriteFailed(String),

    #[error("Watched file not found with id: {0}")]
    WatchedFileNotFound(i64),

    #[error("Snapshot not found with id: {0}")]
    SnapshotNotFound(i64),

    #[error("Change record not found with id: {0}")]
    ChangeNotFound(i64),

    #[error("Database schema mismatch - may need reinitialization")]
    SchemaMismatch,

    #[error("SQLite error: {0}")]
    SqliteError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_serialization() {
        let status = WatchedFile {
            id: 1,
            path: "~/.config/test".to_string(),
            normalized_path: PathBuf::from("/home/user/.config/test"),
            tool: "test".to_string(),
            file_type: "json".to_string(),
            status: "Active".to_string(),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("Active"));
    }

    #[test]
    fn test_change_display_structure() {
        let display = ChangeDisplay {
            id: 1,
            timestamp: SystemTime::now(),
            tool: "wezterm".to_string(),
            file_path: "~/.config/wezterm/wezterm.lua".to_string(),
            change_kind: "updated".to_string(),
            diff_format: "text".to_string(),
            summary_json: "{}".to_string(),
        };

        let json = serde_json::to_string(&display).unwrap();
        assert!(json.contains("wezterm"));
    }
}
