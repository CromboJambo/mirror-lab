use clap::CommandFactory;
use clap_mangen::Man;

#[derive(clap::Parser)]
#[command(name = "mirror-log")]
#[command(about = "Append-only event log with SQLite", long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "mirror.db")]
    db: std::path::PathBuf,

    #[arg(short, long, default_value_t = 1000)]
    batch_size: usize,

    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
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
}

fn main() -> std::io::Result<()> {
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or(std::io::ErrorKind::NotFound)?);
    let cmd = Cli::command();
    let man = Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;
    std::fs::write(out_dir.join("mirror-log.1"), buffer)?;
    Ok(())
}
