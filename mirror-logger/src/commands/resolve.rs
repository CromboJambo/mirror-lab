use crate::entry::MirrorEntry;
use crate::store::{JsonlStore, SqliteStore};
use crate::tri::Tri;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct ResolveArgs {
    /// Entry ID to resolve
    pub id: u64,

    /// New state: pass (+1) or fail (-1)
    #[arg(short, long, default_value = "pass")]
    pub state: String,

    /// Reason for resolution
    #[arg(short, long)]
    pub reason: Option<String>,
}

pub fn resolve_entry(args: ResolveArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Get paths
    let jsonl_path = PathBuf::from("data/mirror.log.jsonl");
    let db_path = PathBuf::from("data/mirror.db");

    // Initialize stores
    let mut jsonl_store = JsonlStore::new(&jsonl_path)?;
    let sqlite_store = SqliteStore::new(&db_path)?;

    // Get parent entry
    let parent_entry = sqlite_store
        .get_by_id(args.id)?
        .ok_or(format!("Entry with ID {} not found", args.id))?;

    // Parse new state
    let new_state = Tri::parse_str(&args.state).ok_or(format!(
        "Invalid state: {}. Must be 'pass' or 'fail'",
        args.state
    ))?;

    // Create new entry referencing parent
    let parent_input = parent_entry.input.clone();
    let parent_state = parent_entry.state;
    let parent_tags = parent_entry.tags.clone();
    let parent_id = parent_entry.id;

    let mut new_entry = MirrorEntry::new(parent_input.clone(), new_state);

    // Add fields
    new_entry = new_entry.with_reason(args.reason.unwrap_or_else(|| {
        format!(
            "Resolved {} from state {}",
            parent_input,
            parent_state.to_str()
        )
    }));

    for tag in parent_tags {
        new_entry = new_entry.with_tag(tag);
    }

    new_entry = new_entry.with_parent(parent_id);

    // Append to JSONL
    jsonl_store.append(&new_entry)?;

    // Insert into SQLite
    let new_id = sqlite_store.insert(&new_entry)?;

    // Update new entry with actual ID
    new_entry.id = new_id;

    println!("✓ Entry resolved successfully!");
    println!("  New ID: {}", new_entry.id);
    println!("  State: {}", new_entry.state.to_str());
    println!("  Parent ID: {}", parent_id);

    Ok(())
}
