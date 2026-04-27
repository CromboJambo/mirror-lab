//! Observer module for a-hole config observer.
//!
//! This module handles:
//! - Foreground watcher lifecycle management
//! - Path expansion and normalization
//! - Debounce policy for rapid save bursts
//! - Event intake from file system watchers
//! - Change detection based on content hashes
//!
//! Follows the MVP operational model: runs as foreground process, not background service.

use crate::capture::{ChangeCapture, EventDebouncer, SnapshotCapture};
use crate::db::Database;
use crate::diff::DiffComputer;
use crate::domain::{DomainError, FileType, WatchStatus};
use anyhow::{Context, Result};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Configuration for the file watcher.
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Debounce duration to prevent rapid event storms.
    pub debounce_duration: Duration,
    /// Whether to watch all default config files.
    pub watch_defaults: bool,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            debounce_duration: Duration::from_millis(500), // 500ms debounce
            watch_defaults: true,
        }
    }
}

/// The foreground file watcher that monitors config files for changes.
pub struct ConfigWatcher {
    /// The underlying notify watcher.
    watcher: Option<RecommendedWatcher>,
    /// Database connection for recording changes.
    db: Arc<Mutex<Database>>,
    /// Configuration options.
    config: WatcherConfig,
    /// List of files currently being watched.
    active_watches: Vec<String>,
}

impl ConfigWatcher {
    /// Creates a new file watcher instance.
    pub fn new(db: Database, config: WatcherConfig) -> Self {
        Self {
            watcher: None,
            db: Arc::new(Mutex::new(db)),
            config,
            active_watches: vec![],
        }
    }

    /// Initializes and starts watching files.
    pub fn start(&mut self) -> Result<()> {
        info!("Starting config watcher");

        // Create the notify watcher with debounce configuration
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                tx.send(res)
                    .unwrap_or_else(|_| warn!("Failed to send watch event"));
            },
            Config::default()
                .with_poll_interval(Duration::from_secs(1))
                .with_compare_contents(true), // Compare file contents for deduplication
        )?;

        // Expand and normalize all paths to watch
        let files_to_watch = self.collect_watched_files()?;

        if files_to_watch.is_empty() {
            warn!("No valid files to watch");
            return Ok(());
        }

        // Subscribe each file to the watcher
        for (path, status) in &files_to_watch {
            match watcher.watch(path, RecursiveMode::NonRecursive) {
                Ok(_) => {
                    self.active_watches.push(path.to_string_lossy().to_string());
                    info!("Now watching: {}", path.display());

                    // Update database with active status
                    if let Some(db) = self.db.lock().ok() {
                        if let Ok(file_id) = db.register_watched_file(
                            &path.to_string_lossy(),
                            path,
                            &Self::infer_tool_from_path(path),
                            FileType::from_extension(path).as_str(),
                        ) {
                            let _ = db.update_watched_file_status(file_id, "Active");
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to watch {}: {}", path.display(), e);

                    // Update database with inactive status
                    if let Some(db) = self.db.lock().ok() {
                        let _ = db.update_watched_file_status(0, "Inactive");
                    }
                }
            }
        }

        self.watcher = Some(watcher);

        info!(
            "Watcher started with {} active watches",
            self.active_watches.len()
        );

        // Start the event loop (blocking)
        self.run_event_loop()?;

        Ok(())
    }

    /// Collects all files to watch, including defaults and explicit paths.
    fn collect_watched_files(&self) -> Result<Vec<(PathBuf, WatchStatus)>> {
        let mut files = vec![];

        if self.config.watch_defaults {
            // Add default watched files from MVP spec
            let home_dir = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());

            let defaults = [
                format!("{}/.config/wezterm/wezterm.lua", home_dir),
                format!("{}/.config/zellij/config.kdl", home_dir),
                format!("{}/.config/nushell/config.nu", home_dir),
                format!("{}/.config/nushell/env.nu", home_dir),
                format!("{}/.config/zed/settings.json", home_dir),
            ];

            for default_path in &defaults {
                let path = PathBuf::from(default_path);
                let status = if path.exists() {
                    WatchStatus::Active
                } else {
                    WatchStatus::Pending
                };
                files.push((path, status));
            }
        }

        Ok(files)
    }

    /// Infers the tool name from a file path.
    fn infer_tool_from_path(path: &PathBuf) -> String {
        let path_str = path.to_string_lossy().to_lowercase();

        if path_str.contains("wezterm") {
            "wezterm".to_string()
        } else if path_str.contains("zellij") {
            "zellij".to_string()
        } else if path_str.contains("nushell") || path_str.ends_with(".nu") {
            "nushell".to_string()
        } else if path_str.contains("zed") {
            "zed".to_string()
        } else if path_str.contains("helix") {
            "helix".to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// Runs the main event loop for file changes.
    fn run_event_loop(&mut self) -> Result<()> {
        let watcher = self.watcher.as_mut().unwrap();

        info!("Entering watch loop - waiting for file changes...");
        println!("\n=== a-hole Config Observer ===");
        println!("Watching {} files:", self.active_watches.len());
        for path in &self.active_watches {
            println!("  {}", path);
        }
        println!("\nPress Ctrl+C to stop watching.\n");

        // Event loop - blocks until watcher stops or error occurs
        loop {
            match watcher.receiver().recv() {
                Ok(event) => {
                    debug!("Received watch event: {:?}", event);
                    self.handle_event(&event)?;
                }
                Err(e) => {
                    warn!("Watcher error: {}", e);

                    // Check if this is a fatal error or recoverable
                    if e.kind() == notify::ErrorKind::WatchNotFound {
                        info!("Watch target no longer exists, stopping watcher");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Handles incoming file system events.
    fn handle_event(&mut self, event: &notify::Event) -> Result<()> {
        // Skip deletion events for now - we focus on content changes
        if matches!(event.kind, notify::EventKind::Remove(_)) {
            debug!("File removed, skipping");
            return Ok(());
        }

        // Process each path in the event
        for path in &event.paths {
            self.process_path_change(path)?;
        }

        Ok(())
    }

    /// Processes a single file path change.
    fn process_path_change(&self, path: &PathBuf) -> Result<()> {
        // Check if this is a watched file
        let path_str = path.to_string_lossy();
        if !self.active_watches.iter().any(|p| p == &path_str) {
            debug!("Unwatched file changed: {}", path.display());
            return Ok(());
        }

        // Get or create debouncer for this path
        let mut watch_info = self.get_watch_info(path)?;

        if !watch_info.debouncer.should_process() {
            debug!(
                "Event debounced ({}ms since last)",
                watch_info.time_since_last().as_millis()
            );
            return Ok(());
        }

        // Read current content and check for material change
        let capturer = SnapshotCapture::new(watch_info.watched_file.clone());

        match capturer.get_current_hash() {
            Ok(current_hash) => {
                if !watch_info.is_material_change(&current_hash) {
                    debug!("Content hash unchanged (metadata-only change)");
                    return Ok(());
                }
            }
            Err(e) => {
                warn!("Failed to read content: {}", e);
                return Ok(());
            }
        }

        // Capture snapshot and record change
        self.capture_and_record_change(path, &watch_info.watched_file)?;

        Ok(())
    }

    /// Gets watch information for a path.
    fn get_watch_info(&self, path: &PathBuf) -> Result<WatchInfo> {
        // Get watched file from database or create new entry
        let db = self
            .db
            .lock()
            .map_err(|e| DomainError::PathResolution(e.to_string()))?;

        // Try to find existing watch entry
        let all_files = db.get_all_watched_files()?;
        let watched_file = all_files
            .iter()
            .find(|f| f.normalized_path == *path)
            .cloned();

        let watched_file = match watched_file {
            Some(f) => f,
            None => {
                // Create new watch entry for unknown file
                let id = db.register_watched_file(
                    &path.to_string_lossy(),
                    path,
                    &Self::infer_tool_from_path(path),
                    FileType::from_extension(path).as_str(),
                )?;

                db.get_watched_file(id)?
                    .context("Failed to retrieve newly created watch entry")?
            }
        };

        // Get previous hash for change detection
        let latest_snapshot = db.get_latest_snapshot(watched_file.id)?;
        let previous_hash = latest_snapshot.map(|s| s.content_hash);

        Ok(WatchInfo {
            watched_file,
            previous_hash,
            debouncer: EventDebouncer::new(Duration::from_millis(500)),
        })
    }

    /// Captures a snapshot and records the change.
    fn capture_and_record_change(
        &self,
        path: &PathBuf,
        watched_file: &crate::db::WatchedFile,
    ) -> Result<()> {
        info!("Processing material change in {}", path.display());

        let db = self
            .db
            .lock()
            .map_err(|e| DomainError::PathResolution(e.to_string()))?;

        // Capture current snapshot
        let capturer = SnapshotCapture::new(watched_file.clone());
        let new_snapshot = match capturer.capture_snapshot() {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to capture snapshot: {}", e);
                return Err(e.into());
            }
        };

        // Get previous snapshot for diff computation
        let prev_snapshot = db.get_latest_snapshot(watched_file.id)?;

        // Compute diff summary
        let old_content = prev_snapshot.as_ref().map(|s| s.content.as_str());
        let new_content = &new_snapshot.content;

        let file_type = FileType::from_extension(path);
        let diff_result = match DiffComputer::compute(old_content, new_content, &file_type) {
            Ok(d) => d,
            Err(e) => {
                warn!("Diff computation failed: {}", e);
                return Ok(()); // Continue with minimal tracking
            }
        };

        // Create current snapshot in database
        let current_snapshot_id = db.create_snapshot(
            watched_file.id,
            &new_snapshot.content,
            &new_snapshot.content_hash,
            Some(new_snapshot.captured_at),
        )?;

        // Record the change
        let previous_snapshot_id = prev_snapshot.as_ref().map(|s| s.id);
        let summary_json = serde_json::to_string(&diff_result.summary).unwrap_or_default();

        let change_id = db.record_change(
            watched_file.id,
            previous_snapshot_id,
            current_snapshot_id,
            new_snapshot.captured_at,
            "updated".to_string(), // MVP: only track updates for now
            diff_result.format.to_string(),
            &summary_json,
            "{}".to_string(), // Empty metadata for v0.1
        )?;

        info!("Recorded change #{} in {}", change_id, path.display());

        // Update watched file timestamp
        let _ = db.update_watched_file_status(watched_file.id, "Active");

        Ok(())
    }
}

/// Information tracked for a single watched file.
struct WatchInfo {
    /// The watched file metadata.
    watched_file: crate::db::WatchedFile,
    /// Previous content hash for change detection.
    previous_hash: Option<String>,
    /// Debouncer to prevent rapid event processing.
    debouncer: EventDebouncer,
}

impl WatchInfo {
    fn is_material_change(&self, current_hash: &str) -> bool {
        match &self.previous_hash {
            Some(prev) => current_hash != prev,
            None => true, // First time seeing this file
        }
    }

    fn time_since_last(&self) -> Duration {
        self.debouncer.time_since_last()
    }
}

/// CLI command to start the watcher.
pub struct WatchCommand;

impl WatchCommand {
    /// Executes the watch command with given database and options.
    pub fn execute(db: Database, watch_defaults: bool) -> Result<()> {
        let config = WatcherConfig {
            debounce_duration: Duration::from_millis(500),
            watch_defaults,
        };

        let mut watcher = ConfigWatcher::new(db, config);
        watcher.start()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_db() -> Database {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        Database::new(Some(db_path)).unwrap()
    }

    #[test]
    fn test_infer_tool_from_wezterm_path() {
        let path = PathBuf::from("/home/user/.config/wezterm/wezterm.lua");
        assert_eq!(ConfigWatcher::infer_tool_from_path(&path), "wezterm");
    }

    #[test]
    fn test_infer_tool_from_nushell_path() {
        let path = PathBuf::from("/home/user/.config/nushell/config.nu");
        assert_eq!(ConfigWatcher::infer_tool_from_path(&path), "nushell");
    }

    #[test]
    fn test_infer_tool_unknown_path() {
        let path = PathBuf::from("/tmp/somefile.conf");
        assert_eq!(ConfigWatcher::infer_tool_from_path(&path), "unknown");
    }

    #[test]
    fn test_watcher_config_defaults() {
        let config = WatcherConfig::default();
        assert_eq!(config.debounce_duration, Duration::from_millis(500));
        assert!(config.watch_defaults);
    }
}
