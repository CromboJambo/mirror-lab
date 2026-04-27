use crate::entry::MirrorEntry;
use crate::store::{JsonlStore, SqliteStore};
use crate::tri::Tri;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct AddArgs {
    /// The input/action to log
    pub input: String,

    /// State: pass (+1), fail (-1), or hold (0)
    #[arg(short, long, default_value = "hold")]
    pub state: String,

    /// Reason for hold or resolved state
    #[arg(short, long)]
    pub reason: Option<String>,

    /// Tags for categorization
    #[arg(short = 't', long)]
    pub tags: Vec<String>,

    /// Parent entry ID for re-evaluation
    #[arg(short = 'p', long)]
    pub parent: Option<u64>,
}

pub fn add_entry(args: AddArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Parse state
    let state = Tri::parse_str(&args.state).ok_or(format!(
        "Invalid state: {}. Must be 'pass', 'fail', or 'hold'",
        args.state
    ))?;

    // Create entry
    let mut entry = MirrorEntry::new(args.input, state);

    // Add optional fields
    if let Some(reason) = args.reason {
        entry = entry.with_reason(reason);
    }

    for tag in args.tags {
        entry = entry.with_tag(tag);
    }

    if let Some(parent) = args.parent {
        entry = entry.with_parent(parent);
    }

    // Get paths
    let jsonl_path = PathBuf::from("data/mirror.log.jsonl");
    let db_path = PathBuf::from("data/mirror.db");

    // Initialize stores
    let mut jsonl_store = JsonlStore::new(&jsonl_path)?;
    let sqlite_store = SqliteStore::new(&db_path)?;

    // Append to JSONL
    jsonl_store.append(&entry)?;

    // Insert into SQLite
    let id = sqlite_store.insert(&entry)?;

    // Update entry with actual ID
    entry.id = id;

    println!("✓ Entry added successfully!");
    println!("  ID: {}", entry.id);
    println!("  State: {}", entry.state.to_str());
    println!("  Input: {}", entry.input);

    Ok(())
}
