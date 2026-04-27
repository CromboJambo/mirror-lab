use arboard::Clipboard;
use rusqlite::params;
use serde_json::json;
use std::error::Error;
use std::rc::Rc;
use std::time::Duration;
use std::time::UNIX_EPOCH;
use uuid::Uuid;

#[cfg(feature = "inference")]
use crate::inference::InferenceBackend;

/// Clipboard watcher that monitors clipboard changes and logs them to the database
pub struct ClipboardWatcher {
    conn: Rc<rusqlite::Connection>,
    last_clip: Option<String>,
    #[cfg(feature = "inference")]
    inference: Option<std::sync::Arc<dyn InferenceBackend>>,
}

impl ClipboardWatcher {
    /// Create a new clipboard watcher
    pub fn new(conn: Rc<rusqlite::Connection>) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            conn,
            last_clip: None,
            #[cfg(feature = "inference")]
            inference: None,
        })
    }

    /// Set an inference backend for content analysis
    #[cfg(feature = "inference")]
    pub fn with_inference(mut self, inference: std::sync::Arc<dyn InferenceBackend>) -> Self {
        self.inference = Some(inference);
        self
    }

    /// Check for clipboard changes and log new content
    pub fn check_and_log(&mut self) -> Result<bool, Box<dyn Error>> {
        let mut clipboard = Clipboard::new()?;
        let current_clip = clipboard.get_text()?;

        // Ignore empty or too short clips
        if current_clip.is_empty() || current_clip.len() < 3 {
            return Ok(false);
        }

        // Check if content changed
        if self.last_clip.as_deref() == Some(current_clip.as_str()) {
            return Ok(false);
        }

        self.last_clip = Some(current_clip.clone());

        // Log the clipboard content
        self.log_content(&current_clip)?;

        Ok(true)
    }

    /// Log content to the database with optional inference
    fn log_content(&self, content: &str) -> Result<(), Box<dyn Error>> {
        let id = Uuid::new_v4().to_string();
        let timestamp = (UNIX_EPOCH.elapsed()?.as_secs() as i64).saturating_sub(1); // Adjust for timezone

        // Check for duplicates
        let content_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            format!("{:x}", hasher.finalize())
        };

        let exists = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM events WHERE content_hash = ?1)",
            [&content_hash],
            |row| row.get::<_, bool>(0),
        )?;

        if exists {
            return Ok(());
        }

        // Prepare metadata
        let meta = {
            #[cfg(feature = "inference")]
            if let Some(ref backend) = self.inference {
                // Optional: Get tags from inference backend
                let tags = backend.tag(content).unwrap_or_default();

                // Prepare JSON metadata
                let mut meta_json = json!({
                    "source": "clipboard",
                    "inference": {
                        "tags": tags,
                    }
                });

                // Optional: Get embedding
                if let Ok(embedding) = backend.embed(content) {
                    meta_json["inference"]["embedding"] = json!(embedding);
                }

                Some(serde_json::to_string(&meta_json)?)
            } else {
                Some(json!({ "source": "clipboard" }).to_string())
            }

            #[cfg(not(feature = "inference"))]
            {
                Some(json!({ "source": "clipboard" }).to_string())
            }
        };

        // Insert the event
        self.conn.execute(
            "INSERT INTO events (id, timestamp, source, content, meta, ingested_at, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                timestamp,
                "clipboard",
                content,
                meta,
                timestamp,
                content_hash
            ],
        )?;

        Ok(())
    }

    /// Run the clipboard watcher in a loop
    pub fn run_loop(&mut self, interval_secs: u64) -> Result<(), Box<dyn Error>> {
        loop {
            if let Err(e) = self.check_and_log() {
                eprintln!("Error checking clipboard: {}", e);
            }
            std::thread::sleep(Duration::from_secs(interval_secs));
        }
    }
}

/// Start a clipboard watcher as a background process
pub fn start_background_watcher(
    db_path: &str,
    interval_secs: u64,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
    let db_path = db_path.to_string();

    let handle = tokio::task::spawn_blocking(move || {
        use crate::db::init_db;
        use crate::sources::clipboard::ClipboardWatcher;

        let conn = match init_db(&db_path) {
            Ok(conn) => Rc::new(conn),
            Err(err) => {
                eprintln!("Failed to open database for clipboard watcher: {}", err);
                return;
            }
        };
        let mut watcher = match ClipboardWatcher::new(conn) {
            Ok(watcher) => watcher,
            Err(err) => {
                eprintln!("Failed to initialize clipboard watcher: {}", err);
                return;
            }
        };

        // Run the watcher
        if let Err(err) = watcher.run_loop(interval_secs) {
            eprintln!("Clipboard watcher stopped: {}", err);
        }
    });

    Ok(handle)
}

/// Get shell setup snippets for clipboard integration
pub fn get_shell_setup() -> String {
    r#"
# Clipboard integration for mirror-log
# Add this to your shell configuration file (e.g., ~/.bashrc, ~/.zshrc, or ~/.config/nushell/env.nu)

# Bash / Zsh integration
if command -v mirror-log &> /dev/null; then
    # Optional: Log clipboard content periodically
    # (uncomment to enable)
    # mirror-log history --watch --interval 60
fi

# Nushell integration
# Add to your env.nu:
# if (which mirror-log) != null {
#     # Optional: Periodic clipboard logging
#     # $env.MIRROR_LOG_CLIPBOARD_INTERVAL = 60
# }
    "#
    .to_string()
}
