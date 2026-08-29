use clap::{CommandFactory, Parser, Subcommand};

/// mirror-log CLI
#[derive(Debug, Parser)]
#[command(name = "mirror-log")]
#[command(about = "Append-only event log with SQLite", long_about = None)]
pub struct Cli {
    #[arg(short, long, default_value = "mirror.db")]
    pub db: std::path::PathBuf,

    #[arg(short, long, default_value_t = 1000)]
    pub batch_size: usize,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Show your attention layer (recently accessed events)
    Attention {
        /// Show flagged items (due for decay)
        #[arg(short, long)]
        flagged: bool,

        /// Show statistics
        #[arg(short, long)]
        stats: bool,
    },

    /// Add an event to the log
    Add {
        /// The content to log
        content: String,

        #[arg(short, long, default_value = "cli")]
        source: String,

        #[arg(short, long)]
        meta: Option<String>,
    },

    /// Add a file's contents as a single event
    AddFile {
        /// Path to the file
        path: std::path::PathBuf,

        #[arg(short, long, default_value = "file")]
        source: String,

        #[arg(short, long)]
        meta: Option<String>,
    },

    /// Append an event directly to the log (no staging/approval)
    Append {
        /// The content to log
        content: String,

        #[arg(short, long, default_value = "cli")]
        source: String,

        #[arg(short, long)]
        meta: Option<String>,
    },

    /// Approve a staged event: append it to the log, then remove the staging file
    Approve {
        /// Staged event ID
        id: String,
    },

    /// Add events from stdin (one per line)
    Stdin {
        #[arg(short, long, default_value = "stdin")]
        source: String,

        #[arg(short, long)]
        meta: Option<String>,
    },

    /// Show ingestion statistics
    Stats,

    /// Show recent events
    Show {
        #[arg(short, long, default_value_t = 20)]
        last: i64,

        #[arg(short, long)]
        source: Option<String>,

        #[arg(short, long)]
        preview: Option<usize>,
    },

    /// Search events by content
    Search {
        /// Search term
        term: String,

        #[arg(short, long)]
        preview: Option<usize>,

        #[arg(long)]
        chunks: bool,
    },

    /// Get a specific event by ID
    Get {
        /// Event ID
        id: String,
    },

    /// Show database info
    Info,

    /// Verify database integrity invariants
    Verify,

    /// Generate embeddings for events in a source (optional feature)
    #[cfg(feature = "embedding")]
    Embed {
        #[arg(short, long, default_value = "cli")]
        source: String,

        #[arg(long, default_value = "token-bucket")]
        model: String,
    },

    /// Search similar events using embeddings (optional feature)
    #[cfg(feature = "embedding")]
    SearchSimilar {
        /// Search term (used to generate query vector)
        term: String,

        #[arg(long, default_value_t = 10)]
        limit: usize,
    },

    /// Add an event to the attention layer
    AddToAttention {
        /// Event ID to add to attention
        event_id: String,
    },

    /// Detect patterns from staged events and propose reflections
    Infer,

    /// Review staged events pending approval
    Review,

    /// Regenerate human.md from declarative base and approved reflections
    Regenerate {
        #[arg(long, default_value = "human.md")]
        output: String,
    },

    /// Create a new session (task scope)
    SessionNew {
        /// Source identifier (e.g., "agent", "user")
        #[arg(long, default_value = "agent")]
        source: String,

        /// Optional summary of what this session is about
        #[arg(short, long)]
        summary: Option<String>,
    },

    /// Close a session (lifecycle marker only; content is untouched)
    SessionEnd {
        /// Session ID
        id: String,
    },

    /// List sessions, newest first
    SessionList {
        #[arg(short, long, default_value_t = 20)]
        limit: i64,
    },

    /// Attach an event to a session (idempotent)
    Attach {
        /// Session ID
        session: String,

        /// Event ID
        event: String,
    },

    /// Record an immutable provenance entry
    Provenance {
        /// Subject kind ("event", "baseline", "reflection", ...)
        #[arg(short, long)]
        kind: String,

        /// Subject identifier (event id, baseline key, ...)
        subject: String,

        /// Why this entry exists
        reason: String,

        /// Where it came from (agent, user, model name, ...)
        #[arg(short, long, default_value = "cli")]
        source: String,

        /// Optional raw event this was derived from
        #[arg(long)]
        event: Option<String>,
    },

    /// Show the provenance lineage for a subject (oldest first)
    ProvenanceShow {
        /// Subject kind
        #[arg(short, long)]
        kind: String,

        /// Subject identifier
        subject: String,
    },

    /// Build a memory-context bundle for an agent (JSON)
    Context {
        /// Scope to a session (optional)
        #[arg(short, long)]
        session: Option<String>,

        /// Number of recent events to include
        #[arg(short, long, default_value_t = 20)]
        limit: i64,
    },
}

pub fn cli() -> clap::Command {
    Cli::command()
}
