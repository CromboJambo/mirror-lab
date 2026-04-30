//! Mirror WIT (WebAssembly Interface Types) Specification
//!
//! This crate defines the component model interface for Mirror modules.
//! Modules are WebAssembly components that communicate via JSONL streams.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     JSONL      ┌──────────────────┐
//! │  mirror-daemon  │◄─────────────►│  mirror-module   │
//! │   (orchestrator)│     JSONL      │  (WASM component)│
//! └─────────────────┘               └──────────────────┘
//!         │                                 │
//!    MirrorTag                    Capability tokens
//! ```

// ============================================================================
// WIT Interface Definition
// ============================================================================

/// package mirror:v0.1.0;
///
/// world mirror-runtime {
///   func init() -> result<record<{ version: u32, tags: list<string> }>, error>;
///   func handle_message(payload: list<u8>) -> result<list<u8>, error>;
///   func get_tags() -> record<{ id: string, permissions: list<string> }>;
/// }

// ============================================================================
// MirrorTag - Capability tokens for module permissions
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MirrorTag {
    /// Reflect: Can create reflections of pipeline executions
    Reflect,

    /// Challenge: Can challenge/verify previous reflections
    Challenge,

    /// Compress: Can compress data streams
    Compress,

    /// Expand: Can decompress/expand data streams
    Expand,

    /// EmpathicLow: Can process low-empathy content (analytical)
    EmpathicLow,

    /// EmpathicHigh: Can process high-empathy content (emotional)
    EmpathicHigh,

    /// Read: Can read from filesystem
    Read,

    /// Write: Can write to filesystem
    Write,

    /// Network: Can access network resources
    Network,
}

// ============================================================================
// Protocol Types - JSONL message format between daemon and modules
// ============================================================================

/// MirrorMessage - Protocol between daemon and modules
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MirrorMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub message_type: MessageType,
    pub payload: serde_json::Value,
}

impl MirrorMessage {
    /// Create a new message with auto-generated ID
    pub fn new(message_type: MessageType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            message_type,
            payload: serde_json::Value::Null,
        }
    }

    /// Encode message to JSONL line
    pub fn encode(&self) -> String {
        serde_json::to_string(self).expect("Failed to encode message")
    }

    /// Decode message from JSONL line
    pub fn decode(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line)
    }
}

/// Message types in the Mirror protocol
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MessageType {
    /// Initialize module and get capabilities
    Init,

    /// Handle a user request
    HandleRequest { data: serde_json::Value },

    /// Filesystem read operation
    FsRead { path: String, offset: u64, len: u32 },

    /// Filesystem write operation
    FsWrite { path: String, data: Vec<u8> },

    /// Check if file exists
    FsExists { path: String },

    /// Get module capabilities
    GetTags,

    /// Response to a message
    Response {
        success: bool,
        data: Option<serde_json::Value>,
        error: Option<String>,
    },
}

/// ModuleInitResponse - Returned from init() call
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModuleInitResponse {
    /// Protocol version (should match daemon's expected version)
    pub version: u32,

    /// MirrorTags this module requires/has
    pub tags: Vec<String>,
}

/// ModuleResponse - Standard response format for handle_message
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModuleResponse {
    /// Success indicator
    pub success: bool,

    /// Response data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,

    /// Error message (if failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ModuleResponse {
    /// Create a successful response
    pub fn ok(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// Create an error response
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }

    /// Encode to JSON for transmission
    pub fn encode(&self) -> String {
        serde_json::to_string(self).expect("Failed to encode response")
    }

    /// Decode from JSON
    pub fn decode(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line)
    }
}

// ============================================================================
// Capability Set - Track module permissions
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct CapabilitySet {
    tags: Vec<MirrorTag>,
}

impl CapabilitySet {
    /// Create a new empty capability set
    pub fn new() -> Self {
        Self { tags: Vec::new() }
    }

    /// Add a tag to the capability set
    pub fn add(&mut self, tag: MirrorTag) {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// Check if all required tags are present
    pub fn has_all(&self, required: &[MirrorTag]) -> bool {
        required.iter().all(|r| self.tags.contains(r))
    }

    /// Convert to string representation for WIT interface
    pub fn as_strings(&self) -> Vec<String> {
        self.tags.iter().map(|t| format!("{:?}", t)).collect()
    }
}

// ============================================================================
// MirrorModule Trait - Interface all modules must implement
// ============================================================================

/// Trait that all Mirror modules should implement
pub trait MirrorModule {
    /// Initialize the module, returning version and tags
    fn init(&mut self) -> Result<ModuleInitResponse, String>;

    /// Handle a message from the daemon (JSONL string)
    fn handle_message(&mut self, payload: &str) -> Result<String, String>;

    /// Get the module's capabilities as MirrorTags
    fn get_tags(&self) -> Vec<MirrorTag>;
}

// ============================================================================
// Example Module Implementation - EchoModule for testing
// ============================================================================

/// Simple echo module for testing
pub struct EchoModule {
    initialized: bool,
}

impl EchoModule {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for EchoModule {
    fn default() -> Self {
        Self::new()
    }
}

impl MirrorModule for EchoModule {
    fn init(&mut self) -> Result<ModuleInitResponse, String> {
        self.initialized = true;
        Ok(ModuleInitResponse {
            version: 1,
            tags: vec!["Read".to_string(), "Write".to_string()],
        })
    }

    fn handle_message(&mut self, payload: &str) -> Result<String, String> {
        if !self.initialized {
            return Err("Module not initialized".to_string());
        }

        // Echo back the payload
        Ok(payload.to_string())
    }

    fn get_tags(&self) -> Vec<MirrorTag> {
        vec![MirrorTag::Read, MirrorTag::Write]
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_encode_decode() {
        let msg = MirrorMessage::new(MessageType::GetTags);
        let encoded = msg.encode();
        let decoded = MirrorMessage::decode(&encoded).unwrap();

        assert_eq!(msg.id, decoded.id);
        assert_eq!(msg.message_type, decoded.message_type);
    }

    #[test]
    fn test_module_response() {
        let response = ModuleResponse::ok(serde_json::json!({"echo": "hello"}));
        let encoded = response.encode();

        let parsed = ModuleResponse::decode(&encoded).unwrap();
        assert!(parsed.success);
        assert_eq!(parsed.data.unwrap()["echo"], "hello");
    }

    #[test]
    fn test_echo_module() {
        let mut module = EchoModule::new();
        let init = module.init().unwrap();

        assert_eq!(init.version, 1);
        assert!(init.tags.contains(&"Read".to_string()));
        assert!(init.tags.contains(&"Write".to_string()));

        let response = module.handle_message(r#"{"test":"data"}"#).unwrap();
        assert_eq!(response, r#"{"test":"data"}"#);
    }

    #[test]
    fn test_capability_set() {
        let mut caps = CapabilitySet::new();
        caps.add(MirrorTag::Read);
        caps.add(MirrorTag::Write);

        assert!(caps.has_all(&[MirrorTag::Read]));
        assert!(caps.has_all(&[MirrorTag::Read, MirrorTag::Write]));
        assert!(!caps.has_all(&[MirrorTag::Network]));
    }
}

// ============================================================================
// WASM Export Examples (for when building for webassembly)
// ============================================================================

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub mod wasm_exports {
    use super::*;

    /// Exported function called by mirror-daemon to initialize
    #[no_mangle]
    pub extern "C" fn init() -> *mut u8 {
        // For WASM, we'd typically return a pointer to a WIT result type
        // This is a simplified example for illustration
        0 as *mut u8
    }

    /// Exported function called by mirror-daemon to handle messages
    #[no_mangle]
    pub extern "C" fn handle_message(payload_ptr: *const u8, payload_len: usize) -> *mut u8 {
        // For WASM, we'd typically return a pointer to a WIT result type
        0 as *mut u8
    }
}
