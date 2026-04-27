//! Database module for a-hole config observer.
//!
//! This module handles:
//! - SQLite connection management
//! - Schema creation and migrations
//! - CRUD operations for watched files, snapshots, and config changes
//! - Query operations for CLI output
//!
//! Follows the MVP data model from kickstart.md exactly.

use crate::db::types::{ConfigChange, DbError, FileSnapshot, RevertRecord, WatchedFile, WatchedFileDisplay};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OpenFlags};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;
use tracing::{debug, error, info, warn};

/// Manages the SQLite database connection and operations.
pub struct Database {
    /// Path to the SQLite database file.
    db_path: PathBuf,
    /// Connection handle (wrapped for thread safety).
    conn: Mutex<Connection>,
}

impl Database {
    /// Creates a new database instance at the given path.
    pub fn new(db_path: Option<PathBuf>) -> Result<Self> {
        let path = db_path.unwrap_or_else(Self::default_db_path);

        info!("Opening database at {}", path.display());

        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .context(format!("Failed to open database at {}", path.display()))?;

        // Verify and create schema if needed
        Self::ensure_schema(&conn)?;

        let db = Self {
            db_path: path,
            conn: Mutex::new(conn),
        };

        info!("Database initialized successfully");
        Ok(db)
    }

    /// Returns the default database path in the user's config directory.
    fn default_db_path() -> PathBuf {
        dirs::config_dir()
            .map(|p| p.join("a-hole").join("observer.db"))
            .unwrap_or_else(|| {
                PathBuf::from("~/.local/share/a-hole/observer.db")
                    .expand_tilde()
                    .unwrap()
            })
    }

    /// Ensures the database schema exists and is up to date.
    fn ensure_schema(conn: &Connection) -> Result<()> {
        // Read and execute schema from embedded SQL
        let schema = include_str!("schema.sql");

        conn.execute_batch(schema)
            .context("Failed to initialize database schema")?;

        debug!("Database schema verified");
        Ok(())
    }

    /// Returns the path to this database file.
    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    // ==================== Watched Files CRUD ====================

    /// Registers a new watched file in the database.
    pub fn register_watched_file(
        &self,
        path: &str,
        normalized_path: &std::path::PathBuf,
        tool: &str,
        file_type: &str,
    ) -> Result<i64> {
        let now = SystemTime::now();

        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::WriteFailed(e.to_string()))?;

        match conn.execute(
            r#"
            INSERT INTO watched_files (path, normalized_path, tool, file_type, status, created_at, updated_at)
            VALUES (?, ?, ?, ?, 'Pending', ?, ?)
            "#,
            params![path, normalized_path.to_string_lossy(), tool, file_type, now, now],
        ) {
            Ok(id) => {
                info!("Registered watched file: {} ({})", path, tool);
                Ok(id)
            }
            Err(e) => {
                error!("Failed to register watched file: {}", e);
                Err(DbError::WriteFailed(format!("Insert failed: {}", e)).into())
            }
        }
    }

    /// Updates the status of a watched file.
    pub fn update_watched_file_status(&self, id: i64, status: &str) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::WriteFailed(e.to_string()))?;

        conn.execute(
            r#"
            UPDATE watched_files SET status = ?, updated_at = ? WHERE id = ?
            "#,
            params![status, SystemTime::now(), id],
        )
        .context("Failed to update watched file status")?;

        info!("Updated watched file {} status to {}", id, status);
        Ok(())
    }

    /// Gets all registered watched files.
    pub fn get_all_watched_files(&self) -> Result<Vec<WatchedFile>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT id, path, normalized_path, tool, file_type, status, created_at, updated_at FROM watched_files")
            .context("Failed to prepare query for watched files")?;

        let rows = stmt.query_map([], |row| {
            Ok(WatchedFile {
                id: row.get(0)?,
                path: row.get(1)?,
                normalized_path: row.get(2)?,
                tool: row.get(3)?,
                file_type: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;

        let files: Result<Vec<WatchedFile>> = rows.collect();
        Ok(files?)
    }

    /// Gets a watched file by ID.
    pub fn get_watched_file(&self, id: i64) -> Result<Option<WatchedFile>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT id, path, normalized_path, tool, file_type, status, created_at, updated_at FROM watched_files WHERE id = ?")
            .context("Failed to prepare query for watched file")?;

        let result = stmt.query_row(params![id], |row| {
            Ok(WatchedFile {
                id: row.get(0)?,
                path: row.get(1)?,
                normalized_path: row.get(2)?,
                tool: row.get(3)?,
                file_type: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        });

        Ok(result.ok())
    }

    /// Gets watched files by tool name.
    pub fn get_watched_files_by_tool(&self, tool: &str) -> Result<Vec<WatchedFile>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT id, path, normalized_path, tool, file_type, status, created_at, updated_at FROM watched_files WHERE tool = ? ORDER BY id")
            .context("Failed to prepare query for watched files by tool")?;

        let rows = stmt.query_map(params![tool], |row| {
            Ok(WatchedFile {
                id: row.get(0)?,
                path: row.get(1)?,
                normalized_path: row.get(2)?,
                tool: row.get(3)?,
                file_type: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;

        let files: Result<Vec<WatchedFile>> = rows.collect();
        Ok(files?)
    }

    /// Lists watched files for CLI display.
    pub fn list_watched_files_display(&self) -> Result<Vec<WatchedFileDisplay>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT id, path, normalized_path, tool, file_type, status FROM watched_files")
            .context("Failed to prepare query for displayed watched files")?;

        let rows = stmt.query_map([], |row| {
            Ok(WatchedFileDisplay {
                id: row.get(0)?,
                path: row.get(1)?,
                normalized_path: row.get(2)?,
                tool: row.get(3)?,
                file_type: row.get(4)?,
                status: row.get(5)?,
            })
        })?;

        let files: Result<Vec<WatchedFileDisplay>> = rows.collect();
        Ok(files?)
    }

    // ==================== Snapshots CRUD ====================

    /// Creates a new snapshot for a watched file.
    pub fn create_snapshot(
        &self,
        watched_file_id: i64,
        content: &str,
        content_hash: &str,
        captured_at: Option<SystemTime>,
    ) -> Result<i64> {
        let now = captured_at.unwrap_or_else(SystemTime::now);

        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::WriteFailed(e.to_string()))?;

        match conn.execute(
            r#"
            INSERT INTO file_snapshots (watched_file_id, content, content_hash, captured_at)
            VALUES (?, ?, ?, ?)
            "#,
            params![watched_file_id, content, content_hash, now],
        ) {
            Ok(id) => {
                debug!(
                    "Created snapshot {} for watched file {}",
                    id, watched_file_id
                );
                Ok(id)
            }
            Err(e) => {
                error!("Failed to create snapshot: {}", e);
                Err(DbError::WriteFailed(format!("Insert failed: {}", e)).into())
            }
        }
    }

    /// Gets the latest snapshot for a watched file.
    pub fn get_latest_snapshot(&self, watched_file_id: i64) -> Result<Option<FileSnapshot>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT id, watched_file_id, content, content_hash, captured_at FROM file_snapshots WHERE watched_file_id = ? ORDER BY captured_at DESC LIMIT 1")
            .context("Failed to prepare query for latest snapshot")?;

        let result = stmt.query_row(params![watched_file_id], |row| {
            Ok(FileSnapshot {
                id: row.get(0)?,
                watched_file_id: row.get(1)?,
                content: row.get(2)?,
                content_hash: row.get(3)?,
                captured_at: row.get(4)?,
            })
        });

        Ok(result.ok())
    }

    /// Gets the previous snapshot before a given snapshot.
    pub fn get_previous_snapshot(&self, current_snapshot_id: i64) -> Result<Option<FileSnapshot>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT id, watched_file_id, content, content_hash, captured_at FROM file_snapshots WHERE id < ? ORDER BY id DESC LIMIT 1")
            .context("Failed to prepare query for previous snapshot")?;

        let result = stmt.query_row(params![current_snapshot_id], |row| {
            Ok(FileSnapshot {
                id: row.get(0)?,
                watched_file_id: row.get(1)?,
                content: row.get(2)?,
                content_hash: row.get(3)?,
                captured_at: row.get(4)?,
            })
        });

        Ok(result.ok())
    }

    /// Gets a snapshot by ID.
    pub fn get_snapshot(&self, id: i64) -> Result<Option<FileSnapshot>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT id, watched_file_id, content, content_hash, captured_at FROM file_snapshots WHERE id = ?")
            .context("Failed to prepare query for snapshot by ID")?;

        let result = stmt.query_row(params![id], |row| {
            Ok(FileSnapshot {
                id: row.get(0)?,
                watched_file_id: row.get(1)?,
                content: row.get(2)?,
                content_hash: row.get(3)?,
                captured_at: row.get(4)?,
            })
        });

        Ok(result.ok())
    }

    /// Gets the snapshot content for a watched file (for revert safety check).
    pub fn get_snapshot_content(&self, id: i64) -> Result<Option<String>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT content FROM file_snapshots WHERE id = ?")
            .context("Failed to prepare query for snapshot content")?;

        let result: Option<String> = stmt.query_row(params![id], |row| row.get(0));
        Ok(result)
    }

    // ==================== Config Changes CRUD ====================

    /// Records a new config change event.
    pub fn record_change(
        &self,
        watched_file_id: i64,
        previous_snapshot_id: Option<i64>,
        current_snapshot_id: i64,
        timestamp: SystemTime,
        change_kind: &str,
        diff_format: &str,
        summary_json: &str,
        metadata_json: &str,
    ) -> Result<i64> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::WriteFailed(e.to_string()))?;

        match conn.execute(
            r#"
            INSERT INTO config_changes (watched_file_id, previous_snapshot_id, current_snapshot_id, timestamp, change_kind, diff_format, summary_json, metadata_json)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                watched_file_id,
                previous_snapshot_id,
                current_snapshot_id,
                timestamp,
                change_kind,
                diff_format,
                summary_json,
                metadata_json
            ],
        ) {
            Ok(id) => {
                info!("Recorded config change #{}", id);
                Ok(id)
            }
            Err(e) => {
                error!("Failed to record config change: {}", e);
                Err(DbError::WriteFailed(format!("Insert failed: {}", e)).into())
            }
        }
    }

    /// Gets recent config changes for display.
    pub fn get_config_changes(&self, limit: Option<usize>) -> Result<Vec<ConfigChange>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let query = if let Some(l) = limit {
            format!(
                "SELECT id, watched_file_id, previous_snapshot_id, current_snapshot_id, timestamp, change_kind, diff_format, summary_json, metadata_json FROM config_changes ORDER BY timestamp DESC LIMIT {}",
                l
            )
        } else {
            "SELECT id, watched_file_id, previous_snapshot_id, current_snapshot_id, timestamp, change_kind, diff_format, summary_json, metadata_json FROM config_changes ORDER BY timestamp DESC".to_string()
        };

        let mut stmt = conn
            .prepare(&query)
            .context("Failed to prepare query for config changes")?;

        let rows = stmt.query_map([], |row| {
            Ok(ConfigChange {
                id: row.get(0)?,
                watched_file_id: row.get(1)?,
                previous_snapshot_id: row.get(2)?,
                current_snapshot_id: row.get(3)?,
                timestamp: row.get(4)?,
                change_kind: row.get(5)?,
                diff_format: row.get(6)?,
                summary_json: row.get(7)?,
                metadata_json: row.get(8)?,
            })
        })?;

        let changes: Result<Vec<ConfigChange>> = rows.collect();
        Ok(changes?)
    }

    /// Gets a specific config change by ID.
    pub fn get_config_change(&self, id: i64) -> Result<Option<ConfigChange>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT id, watched_file_id, previous_snapshot_id, current_snapshot_id, timestamp, change_kind, diff_format, summary_json, metadata_json FROM config_changes WHERE id = ?")
            .context("Failed to prepare query for config change by ID")?;

        let result = stmt.query_row(params![id], |row| {
            Ok(ConfigChange {
                id: row.get(0)?,
                watched_file_id: row.get(1)?,
                previous_snapshot_id: row.get(2)?,
                current_snapshot_id: row.get(3)?,
                timestamp: row.get(4)?,
                change_kind: row.get(5)?,
                diff_format: row.get(6)?,
                summary_json: row.get(7)?,
                metadata_json: row.get(8)?,
            })
        });

        Ok(result.ok())
    }

    /// Gets config changes for a specific watched file.
    pub fn get_config_changes_for_file(&self, watched_file_id: i64) -> Result<Vec<ConfigChange>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT id, watched_file_id, previous_snapshot_id, current_snapshot_id, timestamp, change_kind, diff_format, summary_json, metadata_json FROM config_changes WHERE watched_file_id = ? ORDER BY timestamp DESC")
            .context("Failed to prepare query for config changes by file")?;

        let rows = stmt.query_map(params![watched_file_id], |row| {
            Ok(ConfigChange {
                id: row.get(0)?,
                watched_file_id: row.get(1)?,
                previous_snapshot_id: row.get(2)?,
                current_snapshot_id: row.get(3)?,
                timestamp: row.get(4)?,
                change_kind: row.get(5)?,
                diff_format: row.get(6)?,
                summary_json: row.get(7)?,
                metadata_json: row.get(8)?,
            })
        })?;

        let changes: Result<Vec<ConfigChange>> = rows.collect();
        Ok(changes?)
    }

    // ==================== Revert Records CRUD ====================

    /// Records a revert operation.
    pub fn record_revert(
        &self,
        target_change_id: i64,
        watched_file_id: i64,
        success: bool,
        error_message: Option<String>,
    ) -> Result<i64> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::WriteFailed(e.to_string()))?;

        match conn.execute(
            r#"
            INSERT INTO revert_records (target_change_id, watched_file_id, timestamp, success, error_message)
            VALUES (?, ?, ?, ?, ?)
            "#,
            params![target_change_id, watched_file_id, SystemTime::now(), success, error_message],
        ) {
            Ok(id) => {
                if success {
                    info!("Recorded successful revert #{}", id);
                } else {
                    warn!("Recorded failed revert #{}: {}", id, error_message.unwrap_or_default());
                }
                Ok(id)
            }
            Err(e) => {
                error!("Failed to record revert: {}", e);
                Err(DbError::WriteFailed(format!("Insert failed: {}", e)).into())
            }
        }
    }

    /// Gets recent revert records.
    pub fn get_revert_records(&self, limit: Option<usize>) -> Result<Vec<RevertRecord>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let query = if let Some(l) = limit {
            format!(
                "SELECT id, target_change_id, watched_file_id, timestamp, success, error_message FROM revert_records ORDER BY timestamp DESC LIMIT {}",
                l
            )
        } else {
            "SELECT id, target_change_id, watched_file_id, timestamp, success, error_message FROM revert_records ORDER BY timestamp DESC".to_string()
        };

        let mut stmt = conn
            .prepare(&query)
            .context("Failed to prepare query for revert records")?;

        let rows = stmt.query_map([], |row| {
            Ok(RevertRecord {
                id: row.get(0)?,
                target_change_id: row.get(1)?,
                watched_file_id: row.get(2)?,
                timestamp: row.get(3)?,
                success: row.get(4)?,
                error_message: row.get(5)?,
            })
        })?;

        let records: Result<Vec<RevertRecord>> = rows.collect();
        Ok(records?)
    }

    // ==================== Export/Query Operations ====================

    /// Exports config changes with full details for reporting.
    pub fn export_changes_with_details(&self, limit: Option<usize>) -> Result<Vec<ChangeExport>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let query = format!(
            r#"
            SELECT
                c.id,
                c.timestamp,
                c.change_kind,
                c.diff_format,
                c.summary_json,
                wf.path as watched_path,
                wf.tool,
                s.content as current_content,
                ps.content as previous_content
            FROM config_changes c
            JOIN watched_files wf ON c.watched_file_id = wf.id
            LEFT JOIN file_snapshots s ON c.current_snapshot_id = s.id
            LEFT JOIN file_snapshots ps ON c.previous_snapshot_id = ps.id
            ORDER BY c.timestamp DESC
            "#,
        );

        let mut stmt = conn
            .prepare(&query)
            .context("Failed to prepare export query")?;

        let rows = stmt.query_map([], |row| {
            Ok(ChangeExport {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                change_kind: row.get(2)?,
                diff_format: row.get(3)?,
                summary_json: row.get(4)?,
                watched_path: row.get(5)?,
                tool: row.get(6)?,
                current_content: row.get(7)?,
                previous_content: row.get(8)?,
            })
        })?;

        let exports: Result<Vec<ChangeExport>> = rows.collect();
        Ok(exports?)
    }

    /// Gets file path for a watched file ID.
    pub fn get_file_path_for_watch_id(&self, watched_file_id: i64) -> Result<Option<String>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT path FROM watched_files WHERE id = ?")
            .context("Failed to prepare query for file path")?;

        let result: Option<String> = stmt.query_row(params![watched_file_id], |row| row.get(0));
        Ok(result)
    }
}

/// Export record with full change details.
#[derive(Debug, Clone)]
pub struct ChangeExport {
    pub id: i64,
    pub timestamp: SystemTime,
    pub change_kind: String,
    pub diff_format: String,
    pub summary_json: String,
    pub watched_path: String,
    pub tool: String,
    pub current_content: Option<String>,
    pub previous_content: Option<String>,
}

/// Initializes database with default watched files.
pub fn init_with_defaults(db: &Database) -> Result<()> {
    use std::env;

    let home = env::var("HOME").unwrap_or_else(|_| "~".to_string());

    let default_files = vec![
        (
            format!("{}/.config/wezterm/wezterm.lua", home),
            "wezterm",
            "lua",
        ),
        (
            format!("{}/.config/zellij/config.kdl", home),
            "zellij",
            "kdl",
        ),
        (
            format!("{}/.config/nushell/config.nu", home),
            "nushell",
            "nu",
        ),
        (format!("{}/.config/nushell/env.nu", home), "nushell", "nu"),
        (format!("{}/.config/zed/settings.json", home), "zed", "json"),
    ];

    for (path, tool, file_type) in default_files {
        let normalized = std::fs::canonicalize(&path);
        match &normalized {
            Ok(p) => {
                db.register_watched_file(&path, p, tool, file_type)?;
                db.update_watched_file_status(
                    db.get_all_watched_files()?.last().unwrap().id,
                    "Active",
                )?;
                info!("Successfully registered: {}", path);
            }
            Err(_) => {
                // Register anyway as Pending - user may create it later
                let normalized = PathBuf::from(&path);
                db.register_watched_file(&path, &normalized, tool, file_type)?;
                debug!("Registered pending watch: {}", path);
            }
        }
    }

    info!(
        "Initialized with {} default watched files",
        default_files.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_db() -> (Database, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(Some(db_path.clone())).unwrap();
        (db, temp_dir)
    }

    #[test]
    fn test_register_watched_file() {
        let (db, _temp) = test_db();

        let id = db
            .register_watched_file(
                "~/.config/test.conf",
                &PathBuf::from("/home/user/.config/test.conf"),
                "testtool",
                "json",
            )
            .unwrap();

        assert!(id > 0);

        let file = db.get_watched_file(id).unwrap().unwrap();
        assert_eq!(file.tool, "testtool");
        assert_eq!(file.file_type, "json");
    }

    #[test]
    fn test_create_snapshot() {
        let (db, _temp) = test_db();

        // First register a file
        let file_id = db
            .register_watched_file(
                "~/.config/test.conf",
                &PathBuf::from("/home/user/.config/test.conf"),
                "testtool",
                "json",
            )
            .unwrap();

        // Create snapshot
        let snapshot_id = db
            .create_snapshot(file_id, "test content", "abc123hash", None)
            .unwrap();

        assert!(snapshot_id > 0);

        let snapshot = db.get_snapshot(snapshot_id).unwrap().unwrap();
        assert_eq!(snapshot.content, "test content");
    }

    #[test]
    fn test_record_change() {
        let (db, _temp) = test_db();

        // Register file and create snapshots
        let file_id = db
            .register_watched_file(
                "~/.config/test.conf",
                &PathBuf::from("/home/user/.config/test.conf"),
                "testtool",
                "json",
            )
            .unwrap();

        let prev_snapshot = db
            .create_snapshot(file_id, "old content", "hash1", None)
            .unwrap();
        let curr_snapshot = db
            .create_snapshot(file_id, "new content", "hash2", None)
            .unwrap();

        // Record change
        let change_id = db
            .record_change(
                file_id,
                Some(prev_snapshot),
                curr_snapshot,
                SystemTime::now(),
                "updated",
                "text",
                r#"{"lines_added": 1}"#,
                "{}",
            )
            .unwrap();

        assert!(change_id > 0);

        let change = db.get_config_change(change_id).unwrap().unwrap();
        assert_eq!(change.change_kind, "updated");
    }

    #[test]
    fn test_get_latest_snapshot() {
        let (db, _temp) = test_db();

        let file_id = db
            .register_watched_file(
                "~/.config/test.conf",
                &PathBuf::from("/home/user/.config/test.conf"),
                "testtool",
                "json",
            )
            .unwrap();

        // Create multiple snapshots
        db.create_snapshot(file_id, "content 1", "hash1", None)
            .unwrap();
        let snapshot2 = db
            .create_snapshot(file_id, "content 2", "hash2", None)
            .unwrap();

        let latest = db.get_latest_snapshot(file_id).unwrap().unwrap();
        assert_eq!(latest.id, snapshot2);
    }

    #[test]
    fn test_default_db_path() {
        // Just verify it doesn't panic and returns a valid path
        let path = Database::default_db_path();
        assert!(!path.to_string_lossy().is_empty());
    }
}
