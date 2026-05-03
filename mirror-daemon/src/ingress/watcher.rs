use async_trait::async_trait;
use std::{
    collections::hash_map::DefaultHasher,
    collections::{HashMap, VecDeque},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::sync::Mutex;
use tokio::sync::mpsc::Sender;

use crate::daemon::EventPayload;
use mirror_guard::TrustScore;

const SETTLE_SECS: u64 = 5; // seconds a file must be stable before processing

/// A common interface for all event-producing background tasks.
#[async_trait]
pub trait EventSource: Send + Sync {
    /// Perform one cycle of monitoring and emit events if found.
    async fn poll(&self) -> Result<(), anyhow::Error>;
    /// A human-readable name for the source (e.g., "filesystem", "clipboard").
    fn name(&self) -> &'static str;
}

/// State tracking for individual files monitored by the FileWatcher.
#[derive(Debug)]
pub struct FileState {
    last_size: u64,
    first_seen: std::time::Instant,
    processed: bool,
}

/// Watches a specific directory for new recordings and sends events to the message bus.
pub struct FileWatcher {
    watch_dir: PathBuf,
    extensions: Vec<String>,
    sender: Sender<EventPayload>,
    pending: Arc<Mutex<HashMap<PathBuf, FileState>>>,
    recent_hashes: Arc<Mutex<VecDeque<u64>>>,
}

impl FileWatcher {
    /// Creates a new FileWatcher for the given directory and extensions.
    pub fn new(watch_dir: PathBuf, extensions: Vec<String>, sender: Sender<EventPayload>) -> Self {
        Self {
            watch_dir,
            extensions,
            sender,
            pending: Arc::new(Mutex::new(HashMap::new())),
            recent_hashes: Arc::new(Mutex::new(VecDeque::with_capacity(10))),
        }
    }

    /// Checks if the file state tracker has registered this path as being processed.
    #[allow(dead_code)]
    pub async fn is_already_processed(&self, path: &Path) -> bool {
        self.pending
            .lock()
            .await
            .get(path)
            .is_some_and(|s| s.processed)
    }

    /// Clears the state for a processed path.
    #[allow(dead_code)]
    pub async fn clear_processed(&self, path: &Path) {
        self.pending.lock().await.remove(path);
    }
}

#[async_trait]
impl EventSource for FileWatcher {
    fn name(&self) -> &'static str {
        "filesystem"
    }

    /// Checks the directory for stable, unprocessed recordings and sends an event for each.
    async fn poll(&self) -> Result<(), anyhow::Error> {
        let current_time = std::time::Instant::now();

        // Collect directory entries synchronously (cheap on most systems).
        let entries: Vec<PathBuf> = std::fs::read_dir(&self.watch_dir)
            .map_err(|e| anyhow::anyhow!("Failed to read directory: {}", e))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();

        let mut paths_to_send = Vec::new();

        for path in entries {
            let matches = self
                .extensions
                .iter()
                .any(|ext| path.extension() == Some(ext.as_str().as_ref()));

            if !matches {
                continue;
            }

            let meta = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let current_size = meta.len();

            let mut pending = self.pending.lock().await;
            let state = pending.entry(path.clone()).or_insert(FileState {
                last_size: current_size,
                first_seen: current_time,
                processed: false,
            });

            if state.processed {
                continue;
            }

            let elapsed = current_time.duration_since(state.first_seen).as_secs();
            let size_stable = state.last_size == current_size;

            // Update size for next cycle.
            state.last_size = current_size;

            if size_stable && elapsed >= SETTLE_SECS {
                paths_to_send.push(path);
            }
        }

        for path in paths_to_send {
            let payload = EventPayload {
                pipeline: "obs_recorder".to_string(),
                payload: path.to_string_lossy().into_owned(),
                attempts: 0,
                source_event_id: None,
                trust_layer: 0,
                confidence: TrustScore::new(0.1),
                has_raw_data: true,
                has_uncertainty: true,
                can_interrupt: true,
            };

            match self.sender.try_send(payload.clone()) {
                Ok(()) => {
                    if let Some(state) = self.pending.lock().await.get_mut(&path) {
                        state.processed = true;
                    }
                    let mut hasher = DefaultHasher::new();
                    payload.payload.hash(&mut hasher);
                    let h = hasher.finish();
                    {
                        let mut recent = self.recent_hashes.lock().await;
                        recent.push_back(h);
                        if recent.len() > 10 {
                            recent.pop_front();
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[WARNING] Failed to send event for {:?}. Channel might be full or disconnected: {}",
                        path, e
                    );
                }
            }
        }

        Ok(())
    }
}
