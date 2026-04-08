# Mirror WIT (WebAssembly Interface Types)

The interface specification and code generation tools for Mirror modules.

## Architecture

```
┌─────────────────┐     JSONL      ┌──────────────────┐
│  mirror-daemon  │◄─────────────►│  mirror-module   │
│   (orchestrator)│     JSONL      │  (WASM component)│
└─────────────────┘               └──────────────────┘
         │                                 │
    MirrorTag                    Capability tokens
```

## WIT Interface Specification

The WIT file is located at `src/wit/mirror.wit`:

```wit
package mirror:v0.1.0;

world mirror-runtime {
  func init() -> result<init-response, string>;
  func handle_message(payload: list<u8>) -> result<message-response, string>;
  func get_permissions() -> record<{ id: string, capabilities: list<string> }>;
  func fs_read(path: string, offset: u64, len: u32) -> result<list<u8>, string>;
  func fs_write(path: string, data: list<u8>) -> result<bool, string>;
  func fs_exists(path: string) -> bool;
}
```

## Rust API

### MirrorTag Enum

```rust
pub enum MirrorTag {
    Reflect,
    Challenge,
    Compress,
    Expand,
    EmpathicLow,
    EmpathicHigh,
    Read,
    Write,
    Network,
}
```

### Module Response Types

```rust
pub struct ModuleInitResponse {
    pub version: u32,
    pub tags: Vec<String>,
}

pub struct ModuleResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}
```

### MirrorModule Trait

All modules must implement this trait:

```rust
pub trait MirrorModule {
    fn init(&mut self) -> Result<ModuleInitResponse, String>;
    fn handle_message(&mut self, payload: &str) -> Result<String, String>;
    fn get_tags(&self) -> Vec<MirrorTag>;
}
```

## Example Module

See `examples/echo_module.rs` for a complete example:

```rust
use mirror_wit::{MirrorModule, MirrorTag};

pub struct EchoModule {
    initialized: bool,
}

impl MirrorModule for EchoModule {
    fn init(&mut self) -> Result<ModuleInitResponse, String> {
        Ok(ModuleInitResponse {
            version: 1,
            tags: vec!["Read".to_string(), "Write".to_string()],
        })
    }

    fn handle_message(&mut self, payload: &str) -> Result<String, String> {
        Ok(payload.to_string()) // Echo back
    }

    fn get_tags(&self) -> Vec<MirrorTag> {
        vec![MirrorTag::Read, MirrorTag::Write]
    }
}
```

## Building for WASM

```bash
# Build for WebAssembly target
cargo build --target wasm32-unknown-unknown

# Or use wasm-pack for JavaScript interop
wasm-pack build --target web
```

## Integration with mirror-daemon

The daemon:

1. Loads the module as a WebAssembly component
2. Calls `init()` to get version and capabilities
3. Validates MirrorTag permissions before routing messages
4. Routes JSONL messages via `handle_message()`

## Directory Structure

```
mirror-wit/
├── Cargo.toml              # Main crate dependencies
├── src/
│   ├── lib.rs             # Core types and traits
│   └── wit/
│       └── mirror.wit     # WIT interface specification
├── examples/
│   └── echo_module.rs     # Example module implementation
└── macro/
    ├── Cargo.toml         # Macro crate dependencies
    └── src/
        └── lib.rs         # Procedural macros
```

## Next Steps

1. Generate Rust bindings from WIT: `wit-bindgen mirror.wit`
2. Build modules as WebAssembly components
3. Integrate with `mirror-daemon` for message routing
4. Implement specific modules (filesystem, transforms, etc.)