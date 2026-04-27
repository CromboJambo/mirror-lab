
# Mirror Kernel

A composable, capability-based event processing system with SQLite persistence, designed for audit-ready, immutable event processing with cost-prioritized kernel dispatch.

## Overview

The Mirror Kernel system provides a Rust-first, fully append-only architecture for processing events through a registry of composable kernels. Every event and reflection is immutable, ensuring complete auditability and preventing silent mutation of historical data.

## Key Features

- **Append-only design**: Events and reflections are never mutated, only appended
- **Capability-based execution**: Kernels only execute when they have the required capability tags
- **Contextual cost estimation**: Kernels estimate costs based on their input context
- **Batch traversal**: Efficient processing of large append-only stores with memory-bounded batch processing
- **Provenance tracking**: Every reflection links back to its source events
- **SQLite persistence**: Local, fast, portable, and version-controlled storage
- **Rust-enforced immutability**: Compile-time guarantees prevent silent corruption
- **Audit-ready**: Complete history with timestamps and provenance

## Architecture

### Core Components

```
+----------------+
| Append-only DB |
+----------------+
       |
       v
+--------------------+
| Batch fetch events |
+--------------------+
       |
       v
+--------------------+
| Dispatch kernels   |
| - filter by tags   |
| - estimate cost    |
| - sort by cost     |
+--------------------+
       |
       v
+--------------------+
| Append reflections |
+--------------------+
       |
       v
+--------------------+
| Next batch / end   |
+--------------------+
```

### Core Types

#### MirrorTag
Capability tokens that define what a kernel can do.

```rust
pub enum MirrorTag {
    Reflect,      // General reflection capability
    Challenge,    // Critical questioning capability
    Compress,     // Content compression capability
    Expand,       // Content expansion capability
    EmpathicLow,  // Low-intensity empathy
    EmpathicHigh, // High-intensity empathy
}
```

#### MirrorEvent
Immutable events that can be stored and processed.

```rust
pub struct MirrorEvent {
    pub id: i64,
    pub content: String,
    pub tags: Vec<MirrorTag>,
    pub timestamp: i64,
}
```

#### Reflection
Transformations produced by kernels, linked to source events.

```rust
pub struct Reflection {
    pub new_content: String,
    pub new_tags: Vec<MirrorTag>,
    pub source_event_ids: Vec<i64>,
    pub timestamp: i64,
}
```

#### MirrorKernel
Trait defining a composable kernel.

```rust
pub trait MirrorKernel {
    fn name(&self) -> &str;
    fn transform(&self, events: &[MirrorEvent]) -> Option<Reflection>;
    fn required_tags(&self) -> Vec<MirrorTag>;
    fn estimate_cost(&self, events: &[MirrorEvent]) -> u32; // Contextual cost estimation
}
```

## Installation

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = "0.4"
```

## Quick Start

```rust
use mirror_kernel::{EventStore, KernelRegistry, EmpathicMirror, MirrorEvent, MirrorTag};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create event store
    let event_store = EventStore::new("mirror.db")?;

    // Create registry
    let mut registry = KernelRegistry::new();

    // Register kernel
    registry.register(EmpathicMirror);

    // Append event
    event_store.append_event(&MirrorEvent {
        id: 1,
        content: "User stressed".to_string(),
        tags: vec![MirrorTag::Reflect],
        timestamp: chrono::Utc::now().timestamp(),
    })?;

    // Dispatch and store
    event_store.dispatch_and_store(&registry, &[MirrorTag::Reflect])?;

    Ok(())
}
```

## Database Schema

### Events Table (Append-Only)
```sql
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    tags TEXT NOT NULL -- JSON encoded Vec<MirrorTag>
);
```

### Reflections Table (Append-Only)
```sql
CREATE TABLE IF NOT EXISTS reflections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_event_ids TEXT NOT NULL, -- JSON encoded Vec<i64>
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    tags TEXT NOT NULL -- JSON encoded Vec<MirrorTag>
);
```

## Kernel Development Guide

Create a new kernel by implementing the `MirrorKernel` trait:

```rust
pub struct MyCustomKernel;

impl MirrorKernel for MyCustomKernel {
    fn name(&self) -> &str {
        "my_custom_kernel"
    }

    fn transform(&self, events: &[MirrorEvent]) -> Option<Reflection> {
        // Process events
        let combined = events.iter()
            .map(|e| e.content.clone())
            .collect::<Vec<_>>()
            .join("\n");

        Some(Reflection {
            new_content: format!("My custom transformation: {}", combined),
            new_tags: vec![MirrorTag::Reflect],
            source_event_ids: events.iter().map(|e| e.id).collect(),
            timestamp: chrono::Utc::now().timestamp(),
        })
    }

    fn required_tags(&self) -> Vec<MirrorTag> {
        vec![MirrorTag::Reflect]
    }

    fn estimate_cost(&self, events: &[MirrorEvent]) -> u32 {
        events.len() as u32 * 2 // Lower cost = executed first
    }
}
```

## Available Kernels

### EmpathicMirror
Combines events and adds empathic reflection with high emotional intensity.

```rust
fn estimate_cost(&self, events: &[MirrorEvent]) -> u32 { 5 } // moderate cost
```

### ChallengeMirror
Transforms events into critical challenging questions.

```rust
fn estimate_cost(&self, events: &[MirrorEvent]) -> u32 { 10 } // higher cost
```

### CompressMirror
Shortens event content to a maximum of 50 characters.

```rust
fn estimate_cost(&self, events: &[MirrorEvent]) -> u32 { 3 } // low cost
```

### ExpandMirror
Expands event content with additional context markers.

```rust
fn estimate_cost(&self, events: &[MirrorEvent]) -> u32 { 7 } // moderate cost
```

### DynamicCostMirror
Calculates cost based on total event content length.

```rust
fn estimate_cost(&self, events: &[MirrorEvent]) -> u32 {
    let base = 4;
    let content_length = events.iter().map(|e| e.content.len()).sum::<usize>() as u32;
    base + content_length / 100
} // Contextual cost estimation
```

## Properties

### Immutable Input
Kernels only read `MirrorEvent` slices. Rust enforces no mutation.

### Kernel Capabilities
Only kernels with matching `MirrorTags` execute.

### Auditability
Every reflection stores provenance (source event IDs).

### Composable
You can add hundreds of kernels; dispatch is deterministic.

### Local-First
SQLite is small, fast, portable, inspectable, and version-controlled.

### Rust Confesses at Compile-Time
Impossible to mutate events or violate capabilities silently.

### Contextual Cost Estimation
Cost is estimated based on actual input context, not arbitrary values.

### Cost-Prioritized Execution
Predictable, deterministic execution order.

### Append-Only Traversal
Safe for massive stores with memory-bounded batch processing.

## API Reference

### EventStore

```rust
impl EventStore {
    pub fn new(path: &str) -> Result<Self, RegistryError>
    pub fn append_event(&self, event: &MirrorEvent) -> Result<(), RegistryError>
    pub fn get_events(&self) -> Result<Vec<MirrorEvent>, RegistryError>
    pub fn append_reflection(&self, reflection: &Reflection) -> Result<(), RegistryError>
    pub fn get_reflections(&self) -> Result<Vec<Reflection>, RegistryError>
    pub fn dispatch_and_store(&self, registry: &KernelRegistry, available_tags: &[MirrorTag]) -> Result<(), RegistryError>
    pub fn traverse_and_dispatch(&self, registry: &KernelRegistry, available_tags: &[MirrorTag], batch_size: usize) -> Result<(), RegistryError>
}
```

### KernelRegistry

```rust
impl KernelRegistry {
    pub fn new() -> Self
    pub fn register<K: MirrorKernel + 'static + Send + Sync>(&mut self, kernel: K)
    pub fn dispatch(&self, events: &[MirrorEvent], available_tags: &[MirrorTag]) -> Vec<Reflection>
    pub fn list_kernels(&self) -> Vec<String>
}
```

## License

MPL 2.0