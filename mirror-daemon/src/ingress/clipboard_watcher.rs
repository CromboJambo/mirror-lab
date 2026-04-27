use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::Sender;

use crate::daemon::EventPayload;
use crate::ingress::watcher::EventSource;

/// Watches the system clipboard for changes and sends events to the message bus.
#[allow(dead_code)]
pub struct ClipboardWatcher {
    sender: Sender<EventPayload>,
    last_content: Arc<Mutex<Option<String>>>,
}

#[async_trait]
impl EventSource for ClipboardWatcher {
    fn name(&self) -> &'static str {
        "clipboard"
    }

    /// Checks the clipboard for new content and sends an event if it has changed.
    async fn poll(&self) -> Result<(), anyhow::Error> {
        // Use arboard to access the system clipboard.
        // Note: This requires the `clipboard` feature of mirror-daemon to be enabled.
        #[cfg(feature = "clipboard")]
        {
            use arboard::Clipboard;

            let mut clipboard = Clipboard::new()?;
            let current_content = clipboard.get_text().ok();

            let mut last_content_lock = self.last_content.lock().await;

            // Check if the content has changed since the last poll
            if current_content != *last_content_lock {
                if let Some(text) = current_content {
                    // Update our local state
                    *last_content_lock = Some(text.clone());

                    let payload = EventPayload {
                        pipeline: "clipboard".to_string(),
                        payload: text,
                        attempts: 0,
                    };

                    if let Err(e) = self.sender.try_send(payload) {
                        eprintln!(
                            "[WARNING] Failed to send clipboard event. Channel might be full or disconnected: {}",
                            e
                        );
                    } else {
                        // We don't log success here to avoid flooding logs with every clipboard change,
                        // but the daemon will receive it via the consumer loop.
                    }
                } else {
                    // If clipboard is empty (or doesn't contain text), just update state
                    *last_content_lock = None;
                }
            }
        }

        #[cfg(not(feature = "clipboard"))]
        {
            // If the feature is not enabled, we effectively do nothing.
            // This allows the daemon to run without arboard dependencies if needed.
        }

        Ok(())
    }
}

#[allow(dead_code)]
impl ClipboardWatcher {
    /// Creates a new ClipboardWatcher.
    pub fn new(sender: Sender<EventPayload>) -> Self {
        Self {
            sender,
            last_content: Arc::new(Mutex::new(None)),
        }
    }
}
