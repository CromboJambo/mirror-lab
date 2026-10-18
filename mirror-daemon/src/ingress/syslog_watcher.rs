use async_trait::async_trait;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::Sender;

use crate::daemon::EventPayload;
use crate::ingress::watcher::EventSource;
use mirror_guard::TrustScore;

/// Watches system log sources for service events.
///
/// Supports two backends:
/// - journalctl (systemd systems): polls `journalctl --since` with cursor tracking
/// - File watching (dinit/metalog, legacy syslog): tracks byte offsets per file
///
/// Sends an EventPayload for each new log line, with the source filename/unit
/// as metadata and content derived from parsing the line.
pub struct SyslogWatcher {
    sender: Sender<EventPayload>,
    /// Journal cursor (systemd) or map of path -> offset (file mode)
    journal_cursor: Arc<Mutex<Option<String>>>,
    file_positions: Arc<Mutex<HashMap<PathBuf, u64>>>,
    /// Log files to watch (empty = use journalctl)
    log_files: Vec<PathBuf>,
}

#[async_trait]
impl EventSource for SyslogWatcher {
    fn name(&self) -> &'static str {
        "syslog"
    }

    async fn poll(&self) -> Result<(), anyhow::Error> {
        let sender = self.sender.clone();
        let cursor = self.journal_cursor.clone();
        let positions = self.file_positions.clone();
        let log_files = self.log_files.clone();

        // Spawn blocking read task
        tokio::task::spawn_blocking(move || {
            if log_files.is_empty() {
                // journalctl mode: poll since last cursor
                let use_cursor = {
                    let mut c = cursor.lock().unwrap();
                    c.take()
                };

                let cmd = if let Some(cur) = use_cursor {
                    format!("journalctl --since '{}' -o short-iso 2>/dev/null | tail -100", cur)
                } else {
                    "journalctl --since '5 minutes ago' -o short-iso 2>/dev/null | tail -100".to_string()
                };

                let output = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .output();

                if let Ok(out) = output {
                    let text = String::from_utf8_lossy(&out.stdout);
                    for line in text.lines() {
                        if line.is_empty() {
                            continue;
                        }
                        let mut hasher = DefaultHasher::new();
                        line.hash(&mut hasher);
                        let h = hasher.finish();
                        let source_event_id = format!("evt_journal_{}", h);

                        let payload = EventPayload {
                            pipeline: "syslog_ingest".to_string(),
                            payload: format!("journal\t{}", line.trim_end()),
                            attempts: 0,
                            source_event_id: Some(source_event_id),
                            trust_layer: 3,
                            confidence: TrustScore::new(0.8),
                            has_raw_data: true,
                            has_uncertainty: true,
                            can_interrupt: false,
                        };
                        if sender.try_send(payload).is_err() {
                            eprintln!("[WARNING] Failed to send journal event");
                        }
                    }

                    // Update cursor to now for next poll
                    let now = std::process::Command::new("date")
                        .arg("-u")
                        .arg("+%Y-%m-%dT%H:%M:%S")
                        .output()
                        .ok()
                        .and_then(|o| String::from_utf8(o.stdout).ok())
                        .unwrap_or_default();
                    *cursor.lock().unwrap() = Some(now.trim().to_string());
                }
            } else {
                // File watching mode (original behavior)
                for path in &log_files {
                    let offset = positions.lock().unwrap().get(path).copied().unwrap_or(0);

                    let file = match File::open(path) {
                        Ok(f) => f,
                        Err(_) => continue,
                    };

                    let mut reader = BufReader::new(file);
                    if reader.seek(SeekFrom::Start(offset)).is_err() {
                        continue;
                    }

                    let mut line_buf = String::new();
                    loop {
                        line_buf.clear();
                        match reader.read_line(&mut line_buf) {
                            Ok(0) => break,
                            Ok(_) => {
                                let line_text = line_buf.trim_end().to_string();
                                if line_text.is_empty() {
                                    continue;
                                }

                                let mut hasher = DefaultHasher::new();
                                line_text.hash(&mut hasher);
                                let h = hasher.finish();
                                let source_event_id = format!("evt_file_{}_{}", path.display(), h);

                                let payload = EventPayload {
                                    pipeline: "syslog_ingest".to_string(),
                                    payload: format!("{}\t{}", path.display(), line_text),
                                    attempts: 0,
                                    source_event_id: Some(source_event_id),
                                    trust_layer: 3,
                                    confidence: TrustScore::new(0.8),
                                    has_raw_data: true,
                                    has_uncertainty: true,
                                    can_interrupt: false,
                                };

                                if sender.try_send(payload).is_err() {
                                    eprintln!("[WARNING] Failed to send syslog event");
                                }
                            }
                            Err(_) => break,
                        }
                    }

                    let pos = reader.stream_position().unwrap_or(offset);
                    drop(reader);
                    positions.lock().unwrap().insert(path.clone(), pos);
                }
            }
        })
        .await?;

        Ok(())
    }
}

impl SyslogWatcher {
    /// Create watcher for systemd journal (no file args)
    pub fn new_journal(sender: Sender<EventPayload>) -> Self {
        // Initialize cursor to 5 minutes ago so we don't miss recent events
        let now = std::process::Command::new("date")
            .arg("-u")
            .arg("+%Y-%m-%dT%H:%M:%S")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();

        Self {
            sender,
            journal_cursor: Arc::new(Mutex::new(Some(now.trim().to_string()))),
            file_positions: Arc::new(Mutex::new(HashMap::new())),
            log_files: vec![],
        }
    }

    /// Create watcher for specific log files (dinit/metalog)
    pub fn new(sender: Sender<EventPayload>, log_files: Vec<PathBuf>) -> Self {
        let mut positions = HashMap::new();
        for path in &log_files {
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            positions.insert(path.clone(), size);
        }

        Self {
            sender,
            journal_cursor: Arc::new(Mutex::new(None)),
            file_positions: Arc::new(Mutex::new(positions)),
            log_files,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;
    use tempfile::TempDir;

    #[test]
    fn test_syslog_watcher_creation() {
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("test.log");
        write(&log_path, "").unwrap();

        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let watcher = SyslogWatcher::new(tx, vec![log_path]);
        assert_eq!(watcher.name(), "syslog");
    }

    #[test]
    fn test_journal_watcher_creation() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let watcher = SyslogWatcher::new_journal(tx);
        assert_eq!(watcher.name(), "syslog");
    }
}