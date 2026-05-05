// mirror-log/src/inference/trait.rs
// Inference trait definition for mirror-log inference backends

use serde::{Deserialize, Serialize};

/// Trait for inference backends that can process events and generate metadata
pub trait InferenceBackend {
    /// Generate a summary of the event content
    fn summarize(&self, content: &str) -> Result<String, InferenceError>;

    /// Extract tags from the event content
    fn tag(&self, content: &str) -> Result<Vec<String>, InferenceError>;

    /// Generate embeddings for semantic similarity search
    fn embed(&self, content: &str) -> Result<Vec<f32>, InferenceError>;

    /// Suggest which events should be pinned based on importance
    fn suggest_pins(&self, events: &[Event]) -> Result<Vec<String>, InferenceError>;
}

/// Error type for inference operations
#[derive(Debug, thiserror::Error)]
pub enum InferenceError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Invalid response from inference backend")]
    InvalidResponse,

    #[error("Inference backend not configured")]
    NotConfigured,
}

/// Event type used by inference backends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub content: String,
    pub timestamp: i64,
    pub source: String,
    pub meta: Option<String>,
}

impl Event {
    pub fn new(id: String, content: String, timestamp: i64, source: String) -> Self {
        Self {
            id,
            content,
            timestamp,
            source,
            meta: None,
        }
    }
}

/// Configuration for inference backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Enable/disable inference
    pub enabled: bool,

    /// API endpoint URL (for HTTP backend)
    pub api_url: Option<String>,

    /// API key (if required)
    pub api_key: Option<String>,

    /// Model name to use
    pub model: String,

    /// Embedding dimensions
    pub embedding_dim: usize,

    /// Inference timeout in seconds
    pub timeout: u64,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_url: std::env::var("MIRROR_INFERENCE_URL")
                .ok()
                .map(|v| v.to_string()),
            api_key: None,
            model: "mirror-log-model".to_string(),
            embedding_dim: 1536,
            timeout: 30,
        }
    }
}

/// Stub for future WASM backend
///
/// This is a placeholder for a future implementation that would run
/// inference in a WebAssembly context. The interface contract is:
/// - All methods must be async
/// - Must support WASM memory constraints
/// - Must be compatible with browser environments
///
/// Example implementation stub:
///
/// ```rust
/// #[cfg(target_arch = "wasm32")]
/// impl InferenceBackend for WasmBackend {
///     async fn summarize(&self, content: &str) -> Result<String, InferenceError> {
///         // WASM-specific implementation
///         Ok(content.to_string())
///     }
///
///     // ... other methods
/// }
/// ```
pub trait WasmInferenceBackend: InferenceBackend {
    // Future WASM-specific interface
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = Event::new(
            "test-id".to_string(),
            "test content".to_string(),
            1234567890,
            "test-source".to_string(),
        );
        assert_eq!(event.id, "test-id");
        assert_eq!(event.content, "test content");
    }

    #[test]
    fn test_inference_config_default() {
        let config = InferenceConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.embedding_dim, 1536);
    }
}
