//! HTTP backend for inference using LM Studio OpenAI-compatible API

use super::backend_trait::Event;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::time::Duration;
use thiserror::Error;

/// Error types for HTTP inference operations
#[derive(Debug, Error)]
pub enum HttpError {
    #[error("Request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Configuration for HTTP backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Base URL for the inference API (default: http://localhost:1234/v1)
    pub base_url: String,

    /// API key (optional, for authentication)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Request timeout in seconds (default: 30)
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_timeout() -> u64 {
    30
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            base_url: std::env::var("MIRROR_INFERENCE_URL")
                .unwrap_or_else(|_| "http://localhost:1234/v1".to_string()),
            api_key: None,
            timeout: 30,
        }
    }
}

/// Request payload for inference
#[derive(Debug, Serialize)]
struct InferenceRequest {
    model: String,
    messages: Vec<InferenceMessage>,
}

#[derive(Debug, Serialize)]
struct InferenceMessage {
    role: String,
    content: String,
}

/// Response from inference API
#[derive(Debug, Deserialize)]
struct InferenceResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: String,
}

/// HTTP backend implementation
pub struct HttpBackend {
    client: reqwest::Client,
    config: HttpConfig,
}

impl HttpBackend {
    /// Create a new HTTP backend with configuration
    pub fn new(config: HttpConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout))
            .build()?;

        Self { client, config }
    }

    /// Create a default HTTP backend
    pub fn default_config() -> Self {
        Self::new(HttpConfig::default())
    }

    /// Generate a summary of content
    pub async fn summarize(&self, content: &str) -> Result<String, HttpError> {
        let request = InferenceRequest {
            model: "llama-3.2-3b".to_string(), // Default model, can be configured
            messages: vec![InferenceMessage {
                role: "user".to_string(),
                content: format!("Summarize this in 1-2 sentences: {}", content),
            }],
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(HttpError::InvalidResponse(format!(
                "Request failed with status: {}",
                response.status()
            )));
        }

        let inference: InferenceResponse = response.json().await?;

        Ok(inference.choices[0].message.content.clone())
    }

    /// Extract tags from content
    pub async fn tag(&self, content: &str) -> Result<Vec<String>, HttpError> {
        let request = InferenceRequest {
            model: "llama-3.2-3b".to_string(),
            messages: vec![InferenceMessage {
                role: "user".to_string(),
                content: format!(
                    "Extract 3-5 relevant tags as a comma-separated list (no quotes): {}",
                    content
                ),
            }],
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(HttpError::InvalidResponse(format!(
                "Request failed with status: {}",
                response.status()
            )));
        }

        let inference: InferenceResponse = response.json().await?;

        let tags_str = inference.choices[0].message.content.clone();
        Ok(tags_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }

    /// Generate embedding for content
    pub async fn embed(&self, content: &str) -> Result<Vec<f32>, HttpError> {
        let request = InferenceRequest {
            model: "llama-3.2-3b".to_string(),
            messages: vec![InferenceMessage {
                role: "user".to_string(),
                content: content.to_string(),
            }],
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(HttpError::InvalidResponse(format!(
                "Request failed with status: {}",
                response.status()
            )));
        }

        let _inference: InferenceResponse = response.json().await?;

        // Extract a simplified embedding from the response content
        // In a real implementation, you'd use a proper embedding model
        // For now, we'll return a placeholder vector based on content hash
        let mut hasher = sha2::Sha256::new();
        hasher.update(content.as_bytes());
        let hash = hasher.finalize();

        // Convert hash to float vector (simplified)
        let mut result = Vec::with_capacity(768);
        for i in 0..768 {
            let byte = ((hash[i % 32] as usize) >> ((i % 8) * 8)) as f32;
            result.push(byte);
        }

        Ok(result)
    }

    /// Suggest pins based on events
    pub async fn suggest_pins(&self, events: &[Event]) -> Result<Vec<String>, HttpError> {
        if events.is_empty() {
            return Ok(vec![]);
        }

        // Collect content from events
        let content_list: Vec<String> = events.iter().map(|event| event.content.clone()).collect();

        // Use inference to suggest pins
        let tag_request = InferenceRequest {
            model: "llama-3.2-3b".to_string(),
            messages: vec![InferenceMessage {
                role: "user".to_string(),
                content: format!(
                    "From these events, suggest which should be pinned (most important). Return event IDs in order of importance: {}",
                    content_list.join("; ")
                ),
            }],
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .json(&tag_request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(HttpError::InvalidResponse(format!(
                "Request failed with status: {}",
                response.status()
            )));
        }

        let inference: InferenceResponse = response.json().await?;
        Ok(inference.choices[0]
            .message
            .content
            .lines()
            .flat_map(|line| line.split(','))
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = HttpConfig {
            base_url: "http://localhost:1234/v1".to_string(),
            api_key: Some("test-key".to_string()),
            timeout: 60,
        };

        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: HttpConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.base_url, config.base_url);
        assert_eq!(deserialized.api_key, config.api_key);
        assert_eq!(deserialized.timeout, config.timeout);
    }

    #[test]
    fn test_default_config() {
        let config = HttpConfig::default();
        assert_eq!(config.base_url, "http://localhost:1234/v1");
        assert_eq!(config.timeout, 30);
    }
}
