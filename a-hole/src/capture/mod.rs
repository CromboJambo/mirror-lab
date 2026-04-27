//! Capture module for handling file reads and snapshot creation.
//!
//! This module is responsible for:
//! - Reading file contents safely
//! - Computing content hashes for change detection
//! - Creating snapshots of file state at specific points in time
//! - Looking up previous states when detecting changes

use crate::domain::{DomainError, FileType, Snapshot, WatchedFile};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use tracing::{debug, error, info, warn};

/// Captures the current state of a file and creates a snapshot.
pub struct SnapshotCapture {
    /// The watched file being captured.
    watched_file: WatchedFile,
}

impl SnapshotCapture {
    /// Creates a new capture handler for a watched file.
    pub fn new(watched_file: WatchedFile) -> Self {
        Self { watched_file }
    }

    /// Reads the current content of the watched file.
    pub fn read_content(&self) -> Result<String, CaptureError> {
        match fs::read_to_string(&self.watched_file.normalized_path) {
            Ok(content) => Ok(content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(CaptureError::FileNotFound(
                self.watched_file.normalized_path.clone(),
            )),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Err(
                CaptureError::PermissionDenied(self.watched_file.normalized_path.clone()),
            ),
            Err(e) => Err(CaptureError::ReadFailed(
                self.watched_file.normalized_path.clone(),
                e.to_string(),
            )),
        }
    }

    /// Computes a SHA-256 hash of the content.
    pub fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Checks if the file has changed by comparing hashes.
    pub fn has_changed(&self, previous_hash: &str) -> Result<bool, CaptureError> {
        let content = self.read_content()?;
        let current_hash = Self::compute_hash(&content);
        Ok(current_hash != previous_hash)
    }

    /// Captures the current state and creates a snapshot.
    pub fn capture_snapshot(&self) -> Result<Snapshot, CaptureError> {
        let content = self.read_content()?;
        let hash = Self::compute_hash(&content);
        let captured_at = SystemTime::now();

        debug!(
            "Captured snapshot for {}: hash={}, size={} bytes",
            self.watched_file.normalized_path.display(),
            &hash[..16],
            content.len()
        );

        Ok(Snapshot {
            id: 0, // Will be set by database on insert
            watched_file_id: self.watched_file.id,
            content,
            content_hash: hash,
            captured_at,
        })
    }

    /// Gets the current content hash without capturing a full snapshot.
    pub fn get_current_hash(&self) -> Result<String, CaptureError> {
        let content = self.read_content()?;
        Ok(Self::compute_hash(&content))
    }
}

/// Error types for capture operations.
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Permission denied reading file: {0}")]
    PermissionDenied(PathBuf),

    #[error("Failed to read file {0}: {1}")]
    ReadFailed(PathBuf, String),

    #[error("Hash computation failed: {0}")]
    HashError(String),

    #[error("No previous snapshot exists for comparison")]
    NoPreviousSnapshot,

    #[error("Content unchanged - no action needed")]
    ContentUnchanged,
}

/// Handles the logic of comparing current state with previous snapshots.
pub struct StateComparator {
    watched_file: WatchedFile,
}

impl StateComparator {
    /// Creates a new comparator for a watched file.
    pub fn new(watched_file: WatchedFile) -> Self {
        Self { watched_file }
    }

    /// Determines if this is a material change worth recording.
    /// Returns true if the content actually differs from previous state.
    pub fn is_material_change(&self, current_hash: &str, previous_hash: Option<&str>) -> bool {
        match previous_hash {
            Some(prev) => current_hash != prev,
            None => true, // First time seeing this file, always material
        }
    }

    /// Infers the tool from common path patterns if not already known.
    pub fn infer_tool_from_path(&self) -> Option<String> {
        FileType::infer_tool_from_path(&self.watched_file.normalized_path)
            .or_else(|| Some(self.watched_file.tool.clone()))
    }

    /// Gets a descriptive name for the config based on path and tool.
    pub fn get_config_name(&self) -> String {
        self.watched_file
            .normalized_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    /// Provides a human-readable label for the config location.
    pub fn get_config_label(&self) -> String {
        let path_str = self.watched_file.normalized_path.display();
        if let Some(stripped) = path_str.to_string().strip_prefix("~/") {
            format!("~/{stripped}")
        } else {
            path_str.to_string()
        }
    }
}

/// Handles debouncing of rapid file events.
pub struct EventDebouncer {
    last_capture_time: SystemTime,
    debounce_duration: std::time::Duration,
}

impl EventDebouncer {
    /// Creates a new debouncer with the specified duration.
    pub fn new(debounce_duration: std::time::Duration) -> Self {
        Self {
            last_capture_time: SystemTime::now() - debounce_duration, // Allow first capture immediately
            debounce_duration,
        }
    }

    /// Checks if we should process this event based on the debounce policy.
    pub fn should_process(&mut self) -> bool {
        let now = SystemTime::now();
        let elapsed = now
            .duration_since(self.last_capture_time)
            .unwrap_or_default();

        if elapsed >= self.debounce_duration {
            self.last_capture_time = now;
            true
        } else {
            false
        }
    }

    /// Gets the time since last capture.
    pub fn time_since_last(&self) -> std::time::Duration {
        SystemTime::now()
            .duration_since(self.last_capture_time)
            .unwrap_or_default()
    }
}

/// Captures and records a config change event.
pub struct ChangeCapture<'a> {
    comparator: StateComparator,
    debouncer: &'a mut EventDebouncer,
    debounce_duration: std::time::Duration,
}

impl<'a> ChangeCapture<'a> {
    /// Creates a new change capture handler.
    pub fn new(
        watched_file: WatchedFile,
        debouncer: &'a mut EventDebouncer,
        debounce_duration: std::time::Duration,
    ) -> Self {
        Self {
            comparator: StateComparator::new(watched_file),
            debouncer,
            debounce_duration,
        }
    }

    /// Attempts to capture a change if the debounce policy allows it.
    pub fn try_capture(&mut self) -> Result<bool, CaptureError> {
        if !self.debouncer.should_process() {
            debug!(
                "Event debounced ({}ms since last)",
                self.debouncer.time_since_last().as_millis()
            );
            return Ok(false);
        }

        let current_hash = self.comparator.get_current_hash()?;

        if !self.comparator.is_material_change(&current_hash, None) {
            debug!("File changed but content hash unchanged (metadata-only change)");
            return Ok(false);
        }

        info!(
            "Material change detected in {}",
            self.comparator.get_config_label()
        );
        Ok(true)
    }

    /// Gets the label for this config being monitored.
    pub fn get_label(&self) -> String {
        self.comparator.get_config_label()
    }

    /// Gets the tool name for this config.
    pub fn get_tool(&self) -> String {
        self.comparator
            .infer_tool_from_path()
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_compute_hash_consistency() {
        let content = "test content for hashing";
        let hash1 = SnapshotCapture::compute_hash(content);
        let hash2 = SnapshotCapture::compute_hash(content);
        assert_eq!(hash1, hash2);
        assert_ne!("", &hash1[..]);
    }

    #[test]
    fn test_different_content_different_hashes() {
        let content1 = "content one";
        let content2 = "content two";
        let hash1 = SnapshotCapture::compute_hash(content1);
        let hash2 = SnapshotCapture::compute_hash(content2);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_debouncer_allows_first_immediately() {
        let mut debouncer = EventDebouncer::new(std::time::Duration::from_millis(100));

        // Should allow first capture immediately (initialized with offset)
        assert!(debouncer.should_process());
    }

    #[test]
    fn test_debouncer_blocks_rapid_events() {
        let mut debouncer = EventDebouncer::new(std::time::Duration::from_millis(100));

        // First should process immediately
        assert!(debouncer.should_process());

        // Second should be blocked (within debounce window)
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(!debouncer.should_process());

        // Third after delay should process again
        std::thread::sleep(std::time::Duration::from_millis(60));
        assert!(debouncer.should_process());
    }

    #[test]
    fn test_snapshot_capture_from_file() {
        let temp_file = NamedTempFile::new().unwrap();

        // Write initial content
        std::fs::write(&temp_file, "initial content").unwrap();

        let watched_file = WatchedFile {
            id: 1,
            path: temp_file.path().to_string_lossy().to_string(),
            normalized_path: temp_file.path().to_path_buf(),
            tool: "test".to_string(),
            file_type: FileType::Other,
            status: crate::domain::WatchStatus::Active,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };

        let capturer = SnapshotCapture::new(watched_file);
        let snapshot = capturer.capture_snapshot().unwrap();

        assert_eq!(snapshot.content, "initial content");
        assert!(!snapshot.content_hash.is_empty());
    }

    #[test]
    fn test_has_changed_detection() {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(&temp_file, "content v1").unwrap();

        let watched_file = WatchedFile {
            id: 1,
            path: temp_file.path().to_string_lossy().to_string(),
            normalized_path: temp_file.path().to_path_buf(),
            tool: "test".to_string(),
            file_type: FileType::Other,
            status: crate::domain::WatchStatus::Active,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };

        let capturer = SnapshotCapture::new(watched_file);

        // Same content should not show change
        assert!(!capturer.has_changed("samehash").unwrap());

        // Different hash shows change
        assert!(capturer.has_changed("differenthash").unwrap());
    }
}
