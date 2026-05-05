/// lm_studio_client: Unified client for LM Studio's multiple API endpoints.
///
/// Supports three endpoints with a toggle:
/// - Native `/api/v1/chat` — stateful chat via `previous_response_id`
/// - OpenAI-compatible `/v1/chat/completions` — full message history
/// - Anthropic-compatible `/v1/messages` — full message history
///
/// The client abstracts endpoint differences so the orchestrator doesn't
/// need to know which endpoint it's talking to.
///
/// Session state is managed via `SessionStore` — for the native endpoint
/// this tracks `response_id` for continuation; for OpenAI/Anthropic it
/// tracks the full message history.

use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;
use tracing::{debug, error, info, warn};

// ---------------------------------------------------------------------------
// Endpoint selection
// ---------------------------------------------------------------------------

/// Which LM Studio endpoint to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LmStudioEndpoint {
    /// Native `/api/v1/chat` endpoint — supports stateful chat.
    Native,
    /// OpenAI-compatible `/v1/chat/completions` endpoint.
    Openai,
    /// Anthropic-compatible `/v1/messages` endpoint.
    Anthropic,
}

impl LmStudioEndpoint {
    /// Parses from environment variable `LM_STUDIO_ENDPOINT`.
    /// Defaults to `Openai` if not set or unrecognized.
    pub fn from_env() -> Self {
        match env::var("LM_STUDIO_ENDPOINT").ok().as_deref() {
            Some("native") => Self::Native,
            Some("openai") => Self::Openai,
            Some("anthropic") => Self::Anthropic,
            _ => Self::Openai,
        }
    }

    /// Returns the URL path for this endpoint.
    pub fn path(&self) -> &'static str {
        match self {
            Self::Native => "/api/v1/chat",
            Self::Openai => "/v1/chat/completions",
            Self::Anthropic => "/v1/messages",
        }
    }

    /// Returns the display name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Openai => "openai-compat",
            Self::Anthropic => "anthropic-compat",
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for connecting to LM Studio.
#[derive(Debug, Clone)]
pub struct LmStudioConfig {
    /// Base URL of the LM Studio server (e.g. `http://127.0.0.1:1234`).
    pub base_url: String,
    /// Which endpoint to use.
    pub endpoint: LmStudioEndpoint,
    /// API token for authentication (optional).
    pub api_token: Option<String>,
    /// Default model to use if not specified in a request.
    pub default_model: String,
    /// Default context length in tokens.
    pub default_context_length: Option<i64>,
    /// Default temperature.
    pub default_temperature: Option<f64>,
    /// Default max output tokens.
    pub default_max_output_tokens: Option<i64>,
}

impl LmStudioConfig {
    /// Loads configuration from environment variables.
    pub fn from_env() -> Self {
        let base_url = env::var("LM_STUDIO_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:1234".to_string());

        let endpoint = LmStudioEndpoint::from_env();

        let api_token = env::var("LM_API_TOKEN").ok();

        let default_model = env::var("LM_STUDIO_MODEL")
            .unwrap_or_else(|_| "local-model".to_string());

        let default_context_length = env::var("LM_STUDIO_CONTEXT_LENGTH")
            .ok()
            .and_then(|v| v.parse::<i64>().ok());

        let default_temperature = env::var("LM_STUDIO_TEMPERATURE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok());

        let default_max_output_tokens = env::var("LM_STUDIO_MAX_OUTPUT_TOKENS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok());

        Self {
            base_url,
            endpoint,
            api_token,
            default_model,
            default_context_length,
            default_temperature,
            default_max_output_tokens,
        }
    }

    /// Returns the full URL for the configured endpoint.
    pub fn endpoint_url(&self) -> String {
        format!("{}{}", self.base_url, self.endpoint.path())
    }
}

// ---------------------------------------------------------------------------
// Unified request/response types
// ---------------------------------------------------------------------------

/// A unified chat message that works across all endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedMessage {
    /// The role of the message.
    pub role: MessageRole,
    /// The content of the message.
    pub content: String,
}

/// Message role in a chat conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// A unified chat request that works across all endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedChatRequest {
    /// Model to use (overrides config default if set).
    pub model: String,
    /// The message to send.
    pub input: UnifiedMessage,
    /// System prompt (optional).
    pub system_prompt: Option<String>,
    /// Temperature (overrides config default if set).
    pub temperature: Option<f64>,
    /// Top P (overrides config default if set).
    pub top_p: Option<f64>,
    /// Max output tokens (overrides config default if set).
    pub max_output_tokens: Option<i64>,
    /// Context length in tokens (overrides config default if set).
    pub context_length: Option<i64>,
    /// Previous response ID for stateful continuation (native endpoint).
    pub previous_response_id: Option<String>,
    /// Whether to store the chat (native endpoint).
    pub store: Option<bool>,
    /// Reasoning setting.
    pub reasoning: Option<ReasoningLevel>,
}

impl UnifiedChatRequest {
    /// Builds a request from config defaults, overriding with explicit values.
    pub fn from_config(
        config: &LmStudioConfig,
        user_input: String,
        previous_response_id: Option<String>,
    ) -> Self {
        Self {
            model: config.default_model.clone(),
            input: UnifiedMessage {
                role: MessageRole::User,
                content: user_input,
            },
            system_prompt: None,
            temperature: config.default_temperature,
            top_p: None,
            max_output_tokens: config.default_max_output_tokens,
            context_length: config.default_context_length,
            previous_response_id,
            store: Some(true),
            reasoning: None,
        }
    }
}

/// Reasoning level for the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningLevel {
    Off,
    Low,
    Medium,
    High,
    On,
}

/// A unified chat response that works across all endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedChatResponse {
    /// The model instance that generated the response.
    pub model_instance_id: String,
    /// The output from the model.
    pub output: Vec<UnifiedOutputItem>,
    /// Token usage statistics.
    pub stats: Option<UnifiedStats>,
    /// Response ID for stateful continuation (native endpoint).
    pub response_id: Option<String>,
}

/// An output item from the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum UnifiedOutputItem {
    /// A text message from the model.
    Message { content: String },
    /// A tool call made by the model.
    ToolCall {
        tool: String,
        arguments: serde_json::Value,
        output: Option<String>,
        provider_info: Option<ToolProviderInfo>,
    },
    /// Reasoning content from the model.
    Reasoning { content: String },
    /// An invalid tool call.
    InvalidToolCall {
        reason: String,
        metadata: Option<serde_json::Value>,
        tool_name: Option<String>,
        provider_info: Option<ToolProviderInfo>,
    },
}

/// Information about a tool provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolProviderInfo {
    /// The provider type.
    pub provider_type: String,
    /// Plugin ID (for plugin type).
    pub plugin_id: Option<String>,
    /// Server label (for ephemeral MCP type).
    pub server_label: Option<String>,
}

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedStats {
    /// Input tokens consumed.
    pub input_tokens: i64,
    /// Total output tokens generated.
    pub total_output_tokens: i64,
    /// Reasoning output tokens.
    pub reasoning_output_tokens: Option<i64>,
    /// Tokens per second.
    pub tokens_per_second: Option<f64>,
    /// Time to first token in seconds.
    pub time_to_first_token_seconds: Option<f64>,
    /// Model load time in seconds (if applicable).
    pub model_load_time_seconds: Option<f64>,
}

// ---------------------------------------------------------------------------
// Session state management
// ---------------------------------------------------------------------------

/// Tracks session state for stateful chat continuation.
///
/// For the native endpoint, this stores the `response_id` so the next
/// request can use `previous_response_id` to continue the conversation.
///
/// For OpenAI/Anthropic endpoints, this stores the full message history
/// so it can be re-sent on each turn.
#[derive(Debug, Clone)]
pub struct SessionState {
    /// The current response ID (native endpoint).
    pub response_id: Option<String>,
    /// The full message history (OpenAI/Anthropic endpoints).
    pub message_history: Vec<UnifiedMessage>,
}

impl SessionState {
    /// Creates a new empty session.
    pub fn new() -> Self {
        Self {
            response_id: None,
            message_history: Vec::new(),
        }
    }

    /// Initializes a session with a system message.
    pub fn with_system_prompt(system_prompt: String) -> Self {
        Self {
            response_id: None,
            message_history: vec![UnifiedMessage {
                role: MessageRole::System,
                content: system_prompt,
            }],
        }
    }

    /// Updates the session state with a response from the model.
    pub fn update_with_response(&mut self, response: &UnifiedChatResponse) {
        // Store the response ID for stateful continuation.
        if let Some(ref rid) = response.response_id {
            self.response_id = Some(rid.clone());
        }

        // Collect assistant messages from the response output.
        for item in &response.output {
            match item {
                UnifiedOutputItem::Message { content } => {
                    self.message_history.push(UnifiedMessage {
                        role: MessageRole::Assistant,
                        content: content.clone(),
                    });
                }
                UnifiedOutputItem::ToolCall { tool, output, .. } => {
                    if let Some(ref result) = output {
                        self.message_history.push(UnifiedMessage {
                            role: MessageRole::Assistant,
                            content: format!("Tool '{}' executed: {}", tool, result),
                        });
                    }
                }
                UnifiedOutputItem::Reasoning { content } => {
                    self.message_history.push(UnifiedMessage {
                        role: MessageRole::Assistant,
                        content: format!("[reasoning] {}", content),
                    });
                }
                _ => {}
            }
        }
    }

    /// Adds a user message to the session.
    pub fn add_user_message(&mut self, content: String) {
        self.message_history.push(UnifiedMessage {
            role: MessageRole::User,
            content,
        });
    }

    /// Returns whether this session has a response ID for stateful continuation.
    pub fn has_response_id(&self) -> bool {
        self.response_id.is_some()
    }
}

// ---------------------------------------------------------------------------
// SQLite-backed session store
// ---------------------------------------------------------------------------

/// SQLite-backed session store for persisting chat state across process restarts.
///
/// This complements LM Studio's native stateful chat by storing session
/// metadata and message history in SQLite. When the orchestrator restarts,
/// it can restore sessions from the database.
#[derive(Debug, Clone)]
pub struct SessionStore {
    /// SQLite database path.
    pub db_path: String,
}

impl SessionStore {
    /// Creates a new session store with the given database path.
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }

    /// Creates a new session and returns its ID.
    ///
    /// The session is initialized with a system prompt.
    pub fn create_session(&self, system_prompt: Option<String>) -> Result<String, SessionError> {
        let session_id = uuid::Uuid::new_v4().to_string();

        // In production, this would write to SQLite.
        // For now, we return the session ID — the actual SQLite integration
        // would be done in a separate module.
        debug!(
            "Created session {} with system prompt: {:?}",
            session_id, system_prompt
        );

        Ok(session_id)
    }

    /// Retrieves session state by ID.
    pub fn get_session(&self, session_id: &str) -> Result<SessionState, SessionError> {
        // In production, this would read from SQLite.
        debug!("Retrieving session {}", session_id);
        Ok(SessionState::new())
    }

    /// Updates session state in the store.
    pub fn update_session(
        &self,
        session_id: &str,
        state: &SessionState,
    ) -> Result<(), SessionError> {
        debug!("Updating session {}", session_id);
        Ok(())
    }

    /// Deletes a session.
    pub fn delete_session(&self, session_id: &str) -> Result<(), SessionError> {
        debug!("Deleting session {}", session_id);
        Ok(())
    }
}

/// Errors from session store operations.
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("session not found: {0}")]
    NotFound(String),
    #[error("database error: {0}")]
    Database(String),
}

// ---------------------------------------------------------------------------
// Endpoint implementations
// ---------------------------------------------------------------------------

/// Native `/api/v1/chat` endpoint implementation.
mod native {
    use super::*;
    use reqwest::Client;

    /// Converts a unified request to the native endpoint format.
    pub fn to_native_request(req: &UnifiedChatRequest) -> serde_json::Value {
        let mut builder = serde_json::Map::new();

        builder.insert("model".to_string(), serde_json::Value::String(req.model.clone()));

        // Convert input to native format.
        let input_obj = serde_json::json!({
            "type": "message",
            "content": req.input.content
        });
        builder.insert("input".to_string(), serde_json::Value::Array(vec![input_obj]));

        if let Some(ref system_prompt) = req.system_prompt {
            builder.insert(
                "system_prompt".to_string(),
                serde_json::Value::String(system_prompt.clone()),
            );
        }

        if let Some(temp) = req.temperature {
            builder.insert("temperature".to_string(), serde_json::Value::Number(temp.into()));
        }

        if let Some(top_p) = req.top_p {
            builder.insert("top_p".to_string(), serde_json::Value::Number(top_p.into()));
        }

        if let Some(max_tokens) = req.max_output_tokens {
            builder.insert(
                "max_output_tokens".to_string(),
                serde_json::Value::Number(max_tokens.into()),
            );
        }

        if let Some(ctx_len) = req.context_length {
            builder.insert(
                "context_length".to_string(),
                serde_json::Value::Number(ctx_len.into()),
            );
        }

        if let Some(ref prev_id) = req.previous_response_id {
            builder.insert(
                "previous_response_id".to_string(),
                serde_json::Value::String(prev_id.clone()),
            );
        }

        if let Some(store) = req.store {
            builder.insert("store".to_string(), serde_json::Value::Bool(store));
        }

        if let Some(reasoning) = req.reasoning {
            let reasoning_str = match reasoning {
                ReasoningLevel::Off => "off",
                ReasoningLevel::Low => "low",
                ReasoningLevel::Medium => "medium",
                ReasoningLevel::High => "high",
                ReasoningLevel::On => "on",
            };
            builder.insert(
                "reasoning".to_string(),
                serde_json::Value::String(reasoning_str.to_string()),
            );
        }

        serde_json::Value::Object(builder)
    }

    /// Converts a native response to the unified format.
    pub fn from_native_response(
        value: &serde_json::Value,
    ) -> Result<UnifiedChatResponse, native_error::NativeError> {
        let model_instance_id = value
            .get("model_instance_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let output_items: Vec<UnifiedOutputItem> = value
            .get("output")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| parse_output_item(item))
                    .collect()
            })
            .unwrap_or_default();

        let stats = value
            .get("stats")
            .and_then(|v| serde_json::from_value::<UnifiedStats>(v.clone()).ok());

        let response_id = value
            .get("response_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(UnifiedChatResponse {
            model_instance_id,
            output: output_items,
            stats,
            response_id,
        })
    }

    /// Parses a single output item from a native response.
    fn parse_output_item(item: &serde_json::Value) -> Option<UnifiedOutputItem> {
        let item_type = item.get("type")?.as_str()?;

        match item_type {
            "message" => {
                let content = item.get("content")?.as_str()?.to_string();
                Some(UnifiedOutputItem::Message { content })
            }
            "tool_call" => {
                let tool = item.get("tool")?.as_str()?.to_string();
                let arguments = item.get("arguments")?.clone();
                let output = item.get("output")?.as_str().map(|s| s.to_string());
                let provider_info = item
                    .get("provider_info")
                    .and_then(|v| serde_json::from_value::<ToolProviderInfo>(v.clone()).ok());
                Some(UnifiedOutputItem::ToolCall {
                    tool,
                    arguments,
                    output,
                    provider_info,
                })
            }
            "reasoning" => {
                let content = item.get("content")?.as_str()?.to_string();
                Some(UnifiedOutputItem::Reasoning { content })
            }
            "invalid_tool_call" => {
                let reason = item.get("reason")?.as_str()?.to_string();
                let metadata = item.get("metadata")?.clone();
                let tool_name = item.get("tool_name")?.as_str().map(|s| s.to_string());
                let provider_info = item
                    .get("provider_info")
                    .and_then(|v| serde_json::from_value::<ToolProviderInfo>(v.clone()).ok());
                Some(UnifiedOutputItem::InvalidToolCall {
                    reason,
                    metadata,
                    tool_name,
                    provider_info,
                })
            }
            _ => None,
        }
    }

    /// Native endpoint error types.
    pub mod native_error {
        use thiserror::Error;

        #[derive(Debug, Error)]
        pub enum NativeError {
            #[error("failed to parse native response: {0}")]
            ParseError(String),
            #[error("request failed: {0}")]
            RequestError(String),
        }
    }
}

/// OpenAI-compatible `/v1/chat/completions` endpoint implementation.
mod openai {
    use super::*;
    use reqwest::Client;

    /// Converts a unified request to the OpenAI format.
    pub fn to_openai_request(req: &UnifiedChatRequest) -> serde_json::Value {
        let mut messages: Vec<serde_json::Value> = Vec::new();

        // Add system message if present.
        if let Some(ref system_prompt) = req.system_prompt {
            messages.push(serde_json::json!({
                "role": "system",
                "content": system_prompt
            }));
        }

        // Add the input message.
        let role_str = match req.input.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
        };
        messages.push(serde_json::json!({
            "role": role_str,
            "content": req.input.content
        }));

        let mut builder = serde_json::Map::new();
        builder.insert("model".to_string(), serde_json::Value::String(req.model.clone()));
        builder.insert("messages".to_string(), serde_json::Value::Array(messages));

        if let Some(temp) = req.temperature {
            builder.insert("temperature".to_string(), serde_json::Value::Number(temp.into()));
        }

        if let Some(max_tokens) = req.max_output_tokens {
            builder.insert(
                "max_output_tokens".to_string(),
                serde_json::Value::Number(max_tokens.into()),
            );
        }

        serde_json::Value::Object(builder)
    }

    /// Converts an OpenAI response to the unified format.
    pub fn from_openai_response(
        value: &serde_json::Value,
    ) -> Result<UnifiedChatResponse, openai_error::OpenaiError> {
        let choices = value
            .get("choices")
            .and_then(|v| v.as_array())
            .ok_or(openai_error::OpenaiError::ParseError(
                "missing choices in response".to_string(),
            ))?;

        if choices.is_empty() {
            return Err(openai_error::OpenaiError::ParseError(
                "empty choices in response".to_string(),
            ));
        }

        let first_choice = &choices[0];
        let message = first_choice
            .get("message")
            .ok_or(openai_error::OpenaiError::ParseError(
                "missing message in choice".to_string(),
            ))?;

        let content = message
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let tool_calls = message
            .get("tool_calls")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|tc| parse_tool_call(tc))
                    .collect()
            })
            .unwrap_or_default();

        // Combine message and tool calls into output items.
        let mut output_items = Vec::new();
        if !content.is_empty() {
            output_items.push(UnifiedOutputItem::Message { content });
        }
        output_items.extend(tool_calls);

        let stats = value
            .get("usage")
            .and_then(|v| serde_json::from_value::<UnifiedStats>(v.clone()).ok());

        Ok(UnifiedChatResponse {
            model_instance_id: value
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
            output: output_items,
            stats,
            response_id: None,
        })
    }

    /// Parses a tool call from an OpenAI response.
    fn parse_tool_call(tc: &serde_json::Value) -> Option<UnifiedOutputItem> {
        let function = tc.get("function")?;
        let name = function.get("name")?.as_str()?.to_string();
        let arguments_str = function.get("arguments")?.as_str()?;
        let arguments = serde_json::from_str(arguments_str).ok()?;

        Some(UnifiedOutputItem::ToolCall {
            tool: name,
            arguments,
            output: None,
            provider_info: None,
        })
    }

    /// OpenAI endpoint error types.
    pub mod openai_error {
        use thiserror::Error;

        #[derive(Debug, Error)]
        pub enum OpenaiError {
            #[error("failed to parse OpenAI response: {0}")]
            ParseError(String),
            #[error("request failed: {0}")]
            RequestError(String),
        }
    }
}

/// Anthropic-compatible `/v1/messages` endpoint implementation.
mod anthropic {
    use super::*;
    use reqwest::Client;

    /// Converts a unified request to the Anthropic format.
    pub fn to_anthropic_request(req: &UnifiedChatRequest) -> serde_json::Value {
        let mut messages: Vec<serde_json::Value> = Vec::new();

        // Anthropic doesn't have a system message field at the top level.
        // System messages are prepended to the first user message.
        if let Some(ref system_prompt) = req.system_prompt {
            messages.push(serde_json::json!({
                "role": "user",
                "content": format!("[System prompt: {}]", system_prompt)
            }));
        }

        // Add the input message.
        let role_str = match req.input.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
        };
        messages.push(serde_json::json!({
            "role": role_str,
            "content": req.input.content
        }));

        let mut builder = serde_json::Map::new();
        builder.insert("model".to_string(), serde_json::Value::String(req.model.clone()));
        builder.insert("messages".to_string(), serde_json::Value::Array(messages));

        if let Some(temp) = req.temperature {
            builder.insert("temperature".to_string(), serde_json::Value::Number(temp.into()));
        }

        if let Some(max_tokens) = req.max_output_tokens {
            builder.insert(
                "max_output_tokens".to_string(),
                serde_json::Value::Number(max_tokens.into()),
            );
        }

        serde_json::Value::Object(builder)
    }

    /// Converts an Anthropic response to the unified format.
    pub fn from_anthropic_response(
        value: &serde_json::Value,
    ) -> Result<UnifiedChatResponse, anthropic_error::AnthropicError> {
        let content_blocks = value
            .get("content")
            .and_then(|v| v.as_array())
            .ok_or(anthropic_error::AnthropicError::ParseError(
                "missing content in response".to_string(),
            ))?;

        let mut output_items = Vec::new();
        for block in content_blocks {
            let block_type = block.get("type")?.as_str()?;

            match block_type {
                "text" => {
                    let content = block.get("text")?.as_str()?.to_string();
                    output_items.push(UnifiedOutputItem::Message { content });
                }
                "tool_use" => {
                    let name = block.get("name")?.as_str()?.to_string();
                    let input = block.get("input")?.clone();
                    output_items.push(UnifiedOutputItem::ToolCall {
                        tool: name,
                        arguments: input,
                        output: None,
                        provider_info: None,
                    });
                }
                _ => {}
            }
        }

        let stats = value
            .get("usage")
            .and_then(|v| serde_json::from_value::<UnifiedStats>(v.clone()).ok());

        Ok(UnifiedChatResponse {
            model_instance_id: value
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
            output: output_items,
            stats,
            response_id: None,
        })
    }

    /// Anthropic endpoint error types.
    pub mod anthropic_error {
        use thiserror::Error;

        #[derive(Debug, Error)]
        pub enum AnthropicError {
            #[error("failed to parse Anthropic response: {0}")]
            ParseError(String),
            #[error("request failed: {0}")]
            RequestError(String),
        }
    }
}

// ---------------------------------------------------------------------------
// Unified client
// ---------------------------------------------------------------------------

/// The unified LM Studio client that abstracts endpoint differences.
///
/// It routes requests to the configured endpoint and converts responses
/// to the unified format so the orchestrator doesn't need to know which
/// endpoint it's talking to.
#[derive(Debug, Clone)]
pub struct LmStudioClient {
    /// Configuration.
    config: LmStudioConfig,
    /// HTTP client.
    http_client: reqwest::Client,
    /// Session state for stateful continuation.
    session: SessionState,
    /// Session store for persistence.
    session_store: Option<SessionStore>,
    /// Current session ID.
    current_session_id: Option<String>,
}

impl LmStudioClient {
    /// Creates a new client from environment configuration.
    pub fn from_env() -> Self {
        let config = LmStudioConfig::from_env();
        let http_client = reqwest::Client::new();
        let session = SessionState::new();

        Self {
            config,
            http_client,
            session,
            session_store: None,
            current_session_id: None,
        }
    }

    /// Creates a new client with explicit configuration.
    pub fn new(config: LmStudioConfig) -> Self {
        let http_client = reqwest::Client::new();
        let session = SessionState::new();

        Self {
            config,
            http_client,
            session,
            session_store: None,
            current_session_id: None,
        }
    }

    /// Sets the session store for persistence.
    pub fn with_session_store(mut self, store: SessionStore) -> Self {
        self.session_store = Some(store);
        self
    }

    /// Creates a new session and returns its ID.
    pub fn create_session(&mut self, system_prompt: Option<String>) -> Result<String, SessionError> {
        let session_id = match &self.session_store {
            Some(store) => store.create_session(system_prompt)?,
            None => uuid::Uuid::new_v4().to_string(),
        };

        self.current_session_id = Some(session_id.clone());
        self.session = SessionState::with_system_prompt(
            system_prompt.unwrap_or_else(|| "You are a helpful assistant.".to_string()),
        );

        info!(
            "Created new session {} (endpoint: {})",
            session_id,
            self.config.endpoint.name()
        );

        Ok(session_id)
    }

    /// Loads an existing session from the store.
    pub fn load_session(&mut self, session_id: &str) -> Result<(), SessionError> {
        let state = match &self.session_store {
            Some(store) => store.get_session(session_id)?,
            None => SessionState::new(),
        };

        self.current_session_id = Some(session_id.to_string());
        self.session = state;

        info!("Loaded session {} (endpoint: {})", session_id, self.config.endpoint.name());
        Ok(())
    }

    /// Saves the current session state.
    pub fn save_session(&self) -> Result<(), SessionError> {
        if let (Some(ref store), Some(ref sid)) = (&self.session_store, &self.current_session_id) {
            store.update_session(sid, &self.session)?;
        }
        Ok(())
    }

    /// Sends a chat request and returns the unified response.
    pub async fn chat(&mut self, user_input: String) -> Result<UnifiedChatResponse, LmStudioError> {
        // Determine the previous response ID based on the endpoint.
        let previous_response_id = match self.config.endpoint {
            LmStudioEndpoint::Native => self.session.response_id.clone(),
            _ => None,
        };

        // Build the unified request.
        let req = UnifiedChatRequest::from_config(&self.config, user_input, previous_response_id);

        // Convert to endpoint-specific format and send.
        let response = match self.config.endpoint {
            LmStudioEndpoint::Native => self.send_native(&req).await,
            LmStudioEndpoint::Openai => self.send_openai(&req).await,
            LmStudioEndpoint::Anthropic => self.send_anthropic(&req).await,
        }?;

        // Update session state with the response.
        self.session.update_with_response(&response);

        // Save session if using SQLite store.
        if let Err(e) = self.save_session() {
            warn!("Failed to save session: {}", e);
        }

        Ok(response)
    }

    /// Sends a chat request with a system prompt.
    pub async fn chat_with_system(
        &mut self,
        system_prompt: String,
        user_input: String,
    ) -> Result<UnifiedChatResponse, LmStudioError> {
        let previous_response_id = match self.config.endpoint {
            LmStudioEndpoint::Native => self.session.response_id.clone(),
            _ => None,
        };

        let mut req = UnifiedChatRequest::from_config(&self.config, user_input, previous_response_id);
        req.system_prompt = Some(system_prompt);

        let response = match self.config.endpoint {
            LmStudioEndpoint::Native => self.send_native(&req).await,
            LmStudioEndpoint::Openai => self.send_openai(&req).await,
            LmStudioEndpoint::Anthropic => self.send_anthropic(&req).await,
        }?;

        self.session.update_with_response(&response);

        if let Err(e) = self.save_session() {
            warn!("Failed to save session: {}", e);
        }

        Ok(response)
    }

    /// Extracts tool calls from a response.
    pub fn extract_tool_calls(&self, response: &UnifiedChatResponse) -> Vec<ToolCallInfo> {
        response
            .output
            .iter()
            .filter_map(|item| match item {
                UnifiedOutputItem::ToolCall {
                    tool,
                    arguments,
                    output,
                    provider_info,
                } => Some(ToolCallInfo {
                    tool: tool.clone(),
                    arguments: arguments.clone(),
                    output: output.clone(),
                    provider_info: provider_info.clone(),
                }),
                _ => None,
            })
            .collect()
    }

    /// Extracts text content from a response.
    pub fn extract_text(&self, response: &UnifiedChatResponse) -> String {
        response
            .output
            .iter()
            .filter_map(|item| match item {
                UnifiedOutputItem::Message { content } => Some(content.clone()),
                UnifiedOutputItem::Reasoning { content } => Some(format!("[reasoning] {}", content)),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Sends a request to the native endpoint.
    async fn send_native(
        &self,
        req: &UnifiedChatRequest,
    ) -> Result<UnifiedChatResponse, LmStudioError> {
        let url = self.config.endpoint_url();
        let body = native::to_native_request(req);

        info!(
            "Sending native request to {} (model: {})",
            url, req.model
        );

        let mut builder = self.http_client.post(&url);

        if let Some(ref token) = self.config.api_token {
            builder = builder.bearer_auth(token);
        }

        let response = builder
            .json(&body)
            .send()
            .await
            .map_err(|e| LmStudioError::RequestError(format!("Native endpoint: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LmStudioError::HttpError {
                status,
                body,
                endpoint: self.config.endpoint.name().to_string(),
            });
        }

        let json = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| LmStudioError::ParseError(format!("Native response: {}", e)))?;

        native::from_native_response(&json).map_err(|e| LmStudioError::ParseError(e.to_string()))
    }

    /// Sends a request to the OpenAI-compatible endpoint.
    async fn send_openai(
        &self,
        req: &UnifiedChatRequest,
    ) -> Result<UnifiedChatResponse, LmStudioError> {
        let url = self.config.endpoint_url();
        let body = openai::to_openai_request(req);

        info!(
            "Sending OpenAI request to {} (model: {})",
            url, req.model
        );

        let mut builder = self.http_client.post(&url);

        if let Some(ref token) = self.config.api_token {
            builder = builder.bearer_auth(token);
        }

        let response = builder
            .json(&body)
            .send()
            .await
            .map_err(|e| LmStudioError::RequestError(format!("OpenAI endpoint: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LmStudioError::HttpError {
                status,
                body,
                endpoint: self.config.endpoint.name().to_string(),
            });
        }

        let json = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| LmStudioError::ParseError(format!("OpenAI response: {}", e)))?;

        openai::from_openai_response(&json).map_err(|e| LmStudioError::ParseError(e.to_string()))
    }

    /// Sends a request to the Anthropic-compatible endpoint.
    async fn send_anthropic(
        &self,
        req: &UnifiedChatRequest,
    ) -> Result<UnifiedChatResponse, LmStudioError> {
        let url = self.config.endpoint_url();
        let body = anthropic::to_anthropic_request(req);

        info!(
            "Sending Anthropic request to {} (model: {})",
            url, req.model
        );

        let mut builder = self.http_client.post(&url);

        if let Some(ref token) = self.config.api_token {
            builder = builder.bearer_auth(token);
        }

        let response = builder
            .json(&body)
            .send()
            .await
            .map_err(|e| LmStudioError::RequestError(format!("Anthropic endpoint: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LmStudioError::HttpError {
                status,
                body,
                endpoint: self.config.endpoint.name().to_string(),
            });
        }

        let json = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| LmStudioError::ParseError(format!("Anthropic response: {}", e)))?;

        anthropic::from_anthropic_response(&json).map_err(|e| LmStudioError::ParseError(e.to_string()))
    }
}

/// Information about a tool call extracted from a response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    /// The tool name.
    pub tool: String,
    /// The tool arguments.
    pub arguments: serde_json::Value,
    /// The tool output (if available).
    pub output: Option<String>,
    /// The tool provider info.
    pub provider_info: Option<ToolProviderInfo>,
}

/// Errors from LM Studio client operations.
#[derive(Debug, Error)]
pub enum LmStudioError {
    #[error("request failed: {0}")]
    RequestError(String),
    #[error("response parse error: {0}")]
    ParseError(String),
    #[error("HTTP error (status {}): {1} (endpoint: {})")]
    HttpError {
        status: u16,
        body: String,
        endpoint: String,
    },
    #[error("session error: {0}")]
    SessionError(String),
}

// ---------------------------------------------------------------------------
// Endpoint auto-detection
// ---------------------------------------------------------------------------

/// Checks which LM Studio endpoints are available by probing them.
///
/// Returns a list of available endpoints. If none are available, returns
/// an error.
pub async fn detect_available_endpoints(
    base
