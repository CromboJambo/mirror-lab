//! Domain types for a-hole config observer.
//!
//! These types represent the core entities in the system and are shared across modules.
//! They follow the MVP data model from kickstart.md.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

/// Represents a file that is being watched for changes.
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
    pub file_type: FileType,
    /// Current status of the watch target.
    pub status: WatchStatus,
    /// When this file was first registered for watching.
    pub created_at: SystemTime,
    /// Last time a change was recorded for this file.
    pub updated_at: SystemTime,
}

/// Status of a watched file in the system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WatchStatus {
    /// File exists and is being actively monitored.
    Active,
    /// File does not exist yet but may be created later.
    Pending,
    /// File was removed or is inaccessible.
    Inactive,
}

/// The type of config file based on its format/extension.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileType {
    Lua,   // .lua files (wezterm)
    Kdl,   // .kdl files (zellij)
    Nu,    // .nu files (nushell)
    Json,  // .json files (zed settings)
    Toml,  // .toml config files
    Shell, // .sh, .bash scripts
    Ini,   // .ini configuration files
    Other, // Unknown format
}

impl FileType {
    /// Detects the file type based on the file extension.
    pub fn from_extension(path: &PathBuf) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("lua") => FileType::Lua,
            Some("kdl") => FileType::Kdl,
            Some("nu") => FileType::Nu,
            Some("json") => FileType::Json,
            Some("toml") => FileType::Toml,
            Some("sh") | Some("bash") => FileType::Shell,
            Some("ini") => FileType::Ini,
            _ => FileType::Other,
        }
    }

    /// Returns a human-readable name for the file type.
    pub fn as_str(&self) -> &'static str {
        match self {
            FileType::Lua => "lua",
            FileType::Kdl => "kdl",
            FileType::Nu => "nu",
            FileType::Json => "json",
            FileType::Toml => "toml",
            FileType::Shell => "shell",
            FileType::Ini => "ini",
            FileType::Other => "other",
        }
    }

    /// Returns the default tools associated with this file type.
    pub fn default_tools(&self) -> Vec<&'static str> {
        match self {
            FileType::Lua => vec!["wezterm"],
            FileType::Kdl => vec!["zellij"],
            FileType::Nu => vec!["nushell"],
            FileType::Json => vec!["zed", "vscode"],
            FileType::Toml => vec!["helix", "starship"],
            FileType::Shell => vec!["bash", "zsh", "fish"],
            FileType::Ini => vec!["git", "rust-analyzer"],
            FileType::Other => vec![],
        }
    }

    /// Attempts to infer the tool from a common path pattern.
    pub fn infer_tool_from_path(path: &PathBuf) -> Option<String> {
        let path_str = path.to_string_lossy();

        if path_str.contains("wezterm") {
            Some("wezterm".to_string())
        } else if path_str.contains("zellij") {
            Some("zellij".to_string())
        } else if path_str.contains("nushell") || path_str.contains(".nu") {
            Some("nushell".to_string())
        } else if path_str.contains("zed") {
            Some("zed".to_string())
        } else {
            None
        }
    }
}

/// A snapshot of file content at a specific point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Unique identifier for this snapshot.
    pub id: i64,
    /// Reference to the watched file this snapshot belongs to.
    pub watched_file_id: i64,
    /// The full content of the file at capture time.
    pub content: String,
    /// SHA-256 hash of the content for quick change detection.
    pub content_hash: String,
    /// When this snapshot was captured.
    pub captured_at: SystemTime,
}

/// A recorded change event when a watched file is modified.
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
    /// The type of change detected (updated, created, deleted).
    pub change_kind: ChangeKind,
    /// Format used for the diff summary.
    pub diff_format: DiffFormat,
    /// JSON-encoded summary of the change (lines added/removed, key changes).
    pub summary_json: String,
    /// JSON-encoded additional metadata about the change.
    pub metadata_json: String,
}

/// The kind of change that occurred to a watched file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeKind {
    /// File content was modified.
    Updated,
    /// New file was created and is now being watched.
    Created,
    /// File was deleted or moved away.
    Deleted,
}

impl std::fmt::Display for ChangeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeKind::Updated => write!(f, "updated"),
            ChangeKind::Created => write!(f, "created"),
            ChangeKind::Deleted => write!(f, "deleted"),
        }
    }
}

/// The format of the diff summary stored with a change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DiffFormat {
    /// Plain text line-based diff (basic).
    Text,
    /// Semantic diff for JSON/TOML files.
    Json,
    /// Semantic diff for TOML files.
    Toml,
    /// Semantic diff for Lua files.
    Lua,
    /// Parser failed but raw snapshots preserved.
    Unknown,
}

impl std::fmt::Display for DiffFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiffFormat::Text => write!(f, "text"),
            DiffFormat::Json => write!(f, "json"),
            DiffFormat::Toml => write!(f, "toml"),
            DiffFormat::Lua => write!(f, "lua"),
            DiffFormat::Unknown => write!(f, "unknown"),
        }
    }
}

/// A summary of changes computed from comparing snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    /// Total number of lines/keys changed.
    pub total_changes: usize,
    /// Number of lines added.
    pub lines_added: usize,
    /// Number of lines removed.
    pub lines_removed: usize,
    /// Whether the change was material (not trivial).
    pub is_material: bool,
    /// Semantic information about what changed in config keys.
    pub keys_changed: Vec<String>,
}

/// Metadata captured alongside a config change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeMetadata {
    /// Whether the file was readable at capture time.
    pub content_available: bool,
    /// How long the watcher took to detect the change (milliseconds).
    pub detection_latency_ms: Option<u64>,
    /// The editor or tool that made the change if known.
    pub editor_hint: Option<String>,
}

/// A revert operation that restores a prior state.
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

/// Path normalization options for watch targets.
#[derive(Debug, Clone)]
pub struct WatchPathOptions {
    /// Expand ~ to home directory.
    pub expand_tilde: bool,
    /// Resolve relative paths to absolute.
    pub resolve_relative: bool,
    /// Follow symlinks when resolving.
    pub follow_symlinks: bool,
}

impl Default for WatchPathOptions {
    fn default() -> Self {
        Self {
            expand_tilde: true,
            resolve_relative: true,
            follow_symlinks: false,
        }
    }
}

/// Error types for domain operations.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("File type not recognized: {0}")]
    UnknownFileType(String),

    #[error("Invalid watch path: {0}")]
    InvalidWatchPath(String),

    #[error("File does not exist: {0}")]
    FileNotFound(PathBuf),

    #[error("Permission denied accessing file: {0}")]
    PermissionDenied(PathBuf),

    #[error("Cannot watch directory, only files: {0}")]
    WatchDirectory(PathBuf),

    #[error("Path resolution failed: {0}")]
    PathResolution(String),
}

/// Default watched files for the MVP.
pub fn default_watched_files() -> Vec<WatchedFile> {
    use std::env;

    let home = env::var("HOME").unwrap_or_else(|_| "~".to_string());
    let now = SystemTime::now();

    vec![
        WatchedFile {
            id: 0, // Will be set by database on insert
            path: format!("{}/.config/wezterm/wezterm.lua", home),
            normalized_path: PathBuf::from(format!("{}/.config/wezterm/wezterm.lua", home)),
            tool: "wezterm".to_string(),
            file_type: FileType::Lua,
            status: WatchStatus::Pending,
            created_at: now,
            updated_at: now,
        },
        WatchedFile {
            id: 0,
            path: format!("{}/.config/zellij/config.kdl", home),
            normalized_path: PathBuf::from(format!("{}/.config/zellij/config.kdl", home)),
            tool: "zellij".to_string(),
            file_type: FileType::Kdl,
            status: WatchStatus::Pending,
            created_at: now,
            updated_at: now,
        },
        WatchedFile {
            id: 0,
            path: format!("{}/.config/nushell/config.nu", home),
            normalized_path: PathBuf::from(format!("{}/.config/nushell/config.nu", home)),
            tool: "nushell".to_string(),
            file_type: FileType::Nu,
            status: WatchStatus::Pending,
            created_at: now,
            updated_at: now,
        },
        WatchedFile {
            id: 0,
            path: format!("{}/.config/nushell/env.nu", home),
            normalized_path: PathBuf::from(format!("{}/.config/nushell/env.nu", home)),
            tool: "nushell".to_string(),
            file_type: FileType::Nu,
            status: WatchStatus::Pending,
            created_at: now,
            updated_at: now,
        },
        WatchedFile {
            id: 0,
            path: format!("{}/.config/zed/settings.json", home),
            normalized_path: PathBuf::from(format!("{}/.config/zed/settings.json", home)),
            tool: "zed".to_string(),
            file_type: FileType::Json,
            status: WatchStatus::Pending,
            created_at: now,
            updated_at: now,
        },
    ]
}

/// Path utilities for normalizing watch targets.
pub mod path_utils {
    use super::*;

    /// Expands ~ to the home directory and resolves relative paths.
    pub fn normalize_path(path: &str, options: &WatchPathOptions) -> Result<PathBuf, DomainError> {
        let expanded = if options.expand_tilde && path.starts_with('~') {
            match env::var("HOME") {
                Ok(home) => format!("{}{}", home, &path[1..]),
                Err(_) => return Err(DomainError::PathResolution("HOME not set".to_string())),
            }
        } else {
            path.to_string()
        };

        let resolved = if options.resolve_relative {
            let abs_path = std::fs::canonicalize(&expanded);
            match abs_path {
                Ok(p) => p,
                Err(e) => return Err(DomainError::PathResolution(format!("{}", e))),
            }
        } else {
            PathBuf::from(expanded)
        };

        if resolved.is_dir() && options.follow_symlinks {
            // Will be checked by caller for file vs directory
        }

        Ok(resolved)
    }

    /// Checks if a path is accessible and readable.
    pub fn check_accessibility(path: &PathBuf) -> Result<(), DomainError> {
        if !path.exists() {
            return Err(DomainError::FileNotFound(path.clone()));
        }

        if path.is_dir() {
            return Err(DomainError::WatchDirectory(path.clone()));
        }

        match std::fs::metadata(path) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                Err(DomainError::PermissionDenied(path.clone()))
            }
            Err(e) => Err(DomainError::PathResolution(format!("{}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_type_from_extension() {
        assert_eq!(FileType::from_path(&"config.lua".into()), FileType::Lua);
        assert_eq!(FileType::from_path(&"settings.json".into()), FileType::Json);
        assert_eq!(FileType::from_path(&"config.kdl".into()), FileType::Kdl);
        assert_eq!(FileType::from_path(&"config.nu".into()), FileType::Nu);
    }

    #[test]
    fn test_change_kind_display() {
        assert_eq!(format!("{}", ChangeKind::Updated), "updated");
        assert_eq!(format!("{}", ChangeKind::Created), "created");
        assert_eq!(format!("{}", ChangeKind::Deleted), "deleted");
    }
}

// Helper function to match the call in tests
impl FileType {
    pub fn from_path(path: &PathBuf) -> Self {
        Self::from_extension(path)
    }
}
