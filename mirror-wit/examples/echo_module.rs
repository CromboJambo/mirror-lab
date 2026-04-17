//! Example Mirror Module - Echo Functionality
//!
//! This demonstrates a simple WASM module that can be loaded by mirror-daemon.
//! The echo module simply returns the same data it receives, useful for testing.
//!
//! # Build for WASM
//!
//! ```bash
//! # First compile to native (for testing)
//! cargo build --target x86_64-unknown-linux-gnu
//!
//! # Then build for WebAssembly
//! wasm-pack build --target web
//! ```
//!
//! # Test
//!
//! ```bash
//! cargo test
//! ```

use mirror_wit::{MirrorModule, MirrorTag};

/// Simple echo module for testing
pub struct EchoModule {
    initialized: bool,
}

impl EchoModule {
    /// Create a new echo module instance
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
    /// Initialize the module, returning version and tags
    fn init(&mut self) -> Result<mirror_wit::ModuleInitResponse, String> {
        self.initialized = true;
        Ok(mirror_wit::ModuleInitResponse {
            version: 1,
            tags: vec!["Read".to_string(), "Write".to_string()],
        })
    }

    /// Handle a message from the daemon - simply echoes back the payload
    fn handle_message(&mut self, payload: &str) -> Result<String, String> {
        if !self.initialized {
            return Err("Module not initialized. Call init() first.".to_string());
        }

        // Echo back exactly what was received
        Ok(payload.to_string())
    }

    /// Get the module's capabilities as MirrorTags
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
    fn test_echo_init() {
        let mut module = EchoModule::new();
        let response = module.init().unwrap();

        assert_eq!(response.version, 1);
        assert!(response.tags.contains(&"Read".to_string()));
        assert!(response.tags.contains(&"Write".to_string()));
    }

    #[test]
    fn test_echo_handle() {
        let mut module = EchoModule::new();
        module.init().unwrap();

        let result = module.handle_message(r#"{"echo":"test"}"#).unwrap();
        assert_eq!(result, r#"{"echo":"test"}"#);
    }

    #[test]
    fn test_echo_tags() {
        let module = EchoModule::new();
        let tags = module.get_tags();

        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&MirrorTag::Read));
        assert!(tags.contains(&MirrorTag::Write));
    }

    #[test]
    fn test_uninitialized_handle() {
        let mut module = EchoModule::new();

        // Should error if init() not called first
        let result = module.handle_message("test");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Module not initialized. Call init() first."
        );
    }
}

// ============================================================================
// WASM Exports (when building for webassembly)
// ============================================================================

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub mod wasm_exports {
    use super::*;
    use std::str;

    /// Exported function called by mirror-daemon to initialize
    #[no_mangle]
    pub extern "C" fn init() -> *mut u8 {
        let mut module = EchoModule::new();

        match module.init() {
            Ok(response) => {
                // Convert response to JSON bytes
                let json = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
                let bytes = json.into_bytes();

                // Allocate on heap and return pointer
                let boxed = Box::new(bytes);
                Box::into_raw(boxed) as *mut u8
            }
            Err(e) => {
                // Return error as UTF-8 bytes
                let bytes = e.into_bytes();
                let boxed = Box::new(bytes);
                Box::into_raw(boxed) as *mut u8
            }
        }
    }

    /// Exported function called by mirror-daemon to handle messages
    #[no_mangle]
    pub extern "C" fn handle_message(payload_ptr: *const u8, payload_len: usize) -> *mut u8 {
        unsafe {
            if payload_ptr.is_null() || payload_len == 0 {
                let error = "Invalid payload pointer or length".to_string();
                let bytes = error.into_bytes();
                return Box::into_raw(Box::new(bytes)) as *mut u8;
            }

            // Convert raw ptr to slice
            let payload_slice = std::slice::from_raw_parts(payload_ptr, payload_len);

            // Convert to string (assumes UTF-8)
            let payload_str = match str::from_utf8(payload_slice) {
                Ok(s) => s,
                Err(_) => {
                    let error = "Payload is not valid UTF-8".to_string();
                    let bytes = error.into_bytes();
                    return Box::into_raw(Box::new(bytes)) as *mut u8;
                }
            };

            // Initialize and handle message
            let mut module = EchoModule::new();

            match module.init() {
                Ok(_) => match module.handle_message(payload_str) {
                    Ok(response) => {
                        let bytes = response.into_bytes();
                        Box::into_raw(Box::new(bytes)) as *mut u8
                    }
                    Err(e) => {
                        let bytes = e.into_bytes();
                        Box::into_raw(Box::new(bytes)) as *mut u8
                    }
                },
                Err(e) => {
                    let bytes = e.into_bytes();
                    Box::into_raw(Box::new(bytes)) as *mut u8
                }
            }
        }
    }

    /// Free memory allocated by init/handle_message
    #[no_mangle]
    pub extern "C" fn free(ptr: *mut u8) {
        unsafe {
            if !ptr.is_null() {
                let _ = Box::from_raw(ptr as *mut Vec<u8>);
            }
        }
    }
}

// ============================================================================
// Native Binary Entry Point (for testing)
// ============================================================================

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn main() {
    println!("Echo Module - Test Mode");
    println!("========================");

    let mut module = EchoModule::new();

    // Initialize
    match module.init() {
        Ok(response) => println!("Initialized: {:?}", response),
        Err(e) => eprintln!("Init error: {}", e),
    }

    // Handle a test message
    let test_msg = r#"{"command":"echo","data":"hello world"}"#;
    match module.handle_message(test_msg) {
        Ok(response) => println!("Response: {}", response),
        Err(e) => eprintln!("Error: {}", e),
    }
}
