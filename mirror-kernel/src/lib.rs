// Mirror Kernel Library
// A composable, capability-based event processing system with SQLite persistence

use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

pub mod git_decision_layer;
pub use git_decision_layer::{DecisionBlob, DecisionTree, GitDecisionLayer, GitError};

pub use mirror_wit::MirrorTag;

/// Immutable event that can be stored and processed
#[derive(Debug, Clone)]
pub struct MirrorEvent {
    pub id: String,
    pub timestamp: i64,
    pub source: String,
    pub content: String,
    pub content_hash: Option<String>,
    pub meta: Option<String>,
}

/// Reflection produced by a kernel
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Reflection {
    pub new_content: String,
    pub new_tags: Vec<MirrorTag>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Errors that can occur in the registry or event store
#[derive(Debug)]
pub enum RegistryError {
    SqliteError(rusqlite::Error),
    KernelNotFound(String),
    SerializationError(String),
}

impl From<rusqlite::Error> for RegistryError {
    fn from(error: rusqlite::Error) -> Self {
        RegistryError::SqliteError(error)
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::SqliteError(err) => write!(f, "SQLite error: {}", err),
            RegistryError::KernelNotFound(name) => write!(f, "Kernel not found: {}", name),
            RegistryError::SerializationError(err) => write!(f, "Serialization error: {}", err),
        }
    }
}

impl std::error::Error for RegistryError {}

/// Trait that defines a Mirror Kernel
pub trait MirrorKernel {
    fn name(&self) -> &str;

    /// Transform events into a new reflection
    fn transform(&self, events: &[MirrorEvent]) -> Option<Reflection>;

    /// Tags required for this kernel to execute
    fn required_tags(&self) -> Vec<MirrorTag>;
}

/// Trait for tracking iteration updates produced by kernels.
pub trait IterationTracker {
    /// Called when a kernel produces a reflection.
    fn on_reflection(&self, conn: &Connection, event_id: &str, reflection: &Reflection);
}

/// Registry that manages all available kernels
pub struct KernelRegistry {
    kernels: HashMap<String, Arc<dyn MirrorKernel + Send + Sync>>,
    tracker: Option<Box<dyn IterationTracker>>,
    conn: Option<Connection>,
}

impl KernelRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            kernels: HashMap::new(),
            tracker: None,
            conn: None,
        }
    }

    /// Set the iteration tracker for this registry with a database connection.
    pub fn set_tracker<T: IterationTracker + 'static>(&mut self, tracker: T, conn: Connection) {
        self.tracker = Some(Box::new(tracker));
        self.conn = Some(conn);
    }

    /// Register a kernel
    pub fn register<K: MirrorKernel + 'static + Send + Sync>(&mut self, kernel: K) {
        self.kernels
            .insert(kernel.name().to_string(), Arc::new(kernel));
    }

    /// Unregister a kernel
    pub fn unregister(&mut self, kernel_name: &str) -> bool {
        self.kernels.remove(kernel_name).is_some()
    }

    /// Dispatch all matching kernels based on available tags
    pub fn dispatch(
        &self,
        events: &[MirrorEvent],
        available_tags: &[MirrorTag],
    ) -> Vec<Reflection> {
        let mut reflections = Vec::new();

        for kernel in self.kernels.values() {
            if kernel
                .required_tags()
                .iter()
                .all(|t| available_tags.contains(t))
                && let Some(r) = kernel.transform(events)
            {
                reflections.push(r.clone());
                // Trigger iteration tracking if a tracker is present
                if let (Some(tracker), Some(conn)) = (&self.tracker, &self.conn) {
                    for event in events {
                        tracker.on_reflection(conn, &event.id, &r);
                    }
                }
            }
        }

        reflections
    }

    /// List all registered kernel names
    pub fn list_kernels(&self) -> Vec<String> {
        self.kernels.keys().cloned().collect()
    }
}

impl Default for KernelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// SQLite-backed event store for persistent storage
pub struct EventStore {
    conn: Connection,
}

/// Default iteration tracker that does nothing.
pub struct NoOpTracker;

impl IterationTracker for NoOpTracker {
    fn on_reflection(&self, _conn: &Connection, _event_id: &str, _reflection: &Reflection) {}
}

impl EventStore {
    /// Creates a new event store from an existing database connection.
    /// This is the preferred method for integration with mirror-log.
    pub fn from_connection(conn: Connection) -> Self {
        Self { conn }
    }

    /// Create a new event store with SQLite database
    pub fn new(path: &str) -> Result<Self, RegistryError> {
        let conn = Connection::open(path).map_err(RegistryError::SqliteError)?;

        // Create events table (append-only) — mirrors mirror-log substrate schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                source TEXT NOT NULL CHECK (length(source) > 0),
                content TEXT NOT NULL,
                meta TEXT,
                ingested_at INTEGER NOT NULL DEFAULT (unixepoch()),
                content_hash TEXT CHECK (content_hash IS NULL OR length(content_hash) = 64)
            )",
            [],
        )?;

        // Create reflections table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS reflections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kernel_name TEXT NOT NULL,
                new_content TEXT NOT NULL,
                new_tags TEXT NOT NULL,
                timestamp INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    /// Append an event to the database
    pub fn append_event(&self, event: &MirrorEvent) -> Result<(), RegistryError> {
        self.conn.execute(
            "INSERT INTO events (id, timestamp, source, content, meta, ingested_at, content_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.id,
                event.timestamp,
                event.source,
                event.content,
                event.meta.clone(),
                event.timestamp,
                event.content_hash.clone(),
            ],
        )?;

        Ok(())
    }

    /// Retrieve all events from the database
    pub fn get_events(&self) -> Result<Vec<MirrorEvent>, RegistryError> {
        let mut events = Vec::new();

        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, source, content, meta, content_hash FROM events ORDER BY timestamp",
        )?;

        let events_iter = stmt.query_map([], |row| {
            Ok(MirrorEvent {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                source: row.get(2)?,
                content: row.get(3)?,
                meta: row.get::<_, Option<String>>(4)?,
                content_hash: row.get::<_, Option<String>>(5)?,
            })
        })?;

        for event in events_iter {
            events.push(event.map_err(RegistryError::SqliteError)?);
        }

        Ok(events)
    }

    /// Append a reflection to the database
    pub fn append_reflection(
        &self,
        kernel_name: &str,
        reflection: &Reflection,
    ) -> Result<(), RegistryError> {
        let tags_json = serde_json::to_string(&reflection.new_tags)
            .map_err(|e| RegistryError::SerializationError(e.to_string()))?;

        self.conn.execute(
            "INSERT INTO reflections (kernel_name, new_content, new_tags, timestamp) VALUES (?1, ?2, ?3, ?4)",
            params![
                kernel_name,
                reflection.new_content,
                tags_json,
                reflection.timestamp.timestamp()
            ],
        )?;

        Ok(())
    }

    /// Retrieve all reflections from the database
    pub fn get_reflections(&self) -> Result<Vec<Reflection>, RegistryError> {
        let mut reflections = Vec::new();

        let mut stmt = self.conn.prepare(
            "SELECT kernel_name, new_content, new_tags, timestamp FROM reflections ORDER BY timestamp",
        )?;

        let reflections_iter = stmt.query_map([], |row| {
            let tags_json: String = row.get(2)?;
            let tags: Vec<MirrorTag> = serde_json::from_str(&tags_json)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let timestamp_secs: i64 = row.get(3)?;
            let timestamp_dt = chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp_secs, 0)
                .ok_or_else(|| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                    "Invalid timestamp in database",
                )))
            })?;
            Ok(Reflection {
                new_content: row.get(1)?,
                new_tags: tags,
                timestamp: timestamp_dt,
            })
        })?;

        for reflection in reflections_iter {
            reflections.push(reflection.map_err(RegistryError::SqliteError)?);
        }

        Ok(reflections)
    }
}

// Module for example kernels
pub mod kernels;

// Re-export commonly used types and traits
pub use kernels::ChallengeMirror;
pub use kernels::CompressMirror;
pub use kernels::DelusionCompiler;
pub use kernels::EmpathicMirror;
pub use kernels::ExpandMirror;
