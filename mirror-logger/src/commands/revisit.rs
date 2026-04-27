use crate::entry::MirrorEntry;
use crate::store::{JsonlStore, SqliteStore};
use crate::tri::Tri;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct RevisitArgs {
    /// Show only hold entries (state = 0)
    #[arg(long)]
    pub holds: bool,
}

pub fn revisit_entries(args: RevisitArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Get paths
    let jsonl_path = PathBuf::from("data/mirror.log.jsonl");
    let db_path = PathBuf::from("data/mirror.db");

    // Initialize stores
    let _jsonl_store = JsonlStore::new(&jsonl_path)?;
    let sqlite_store = SqliteStore::new(&db_path)?;

    // Get entries
    let entries = if args.holds {
        sqlite_store.get_holds()?
    } else {
        sqlite_store.get_all()?
    };

    // Filter for hold entries (state = 0)
    let hold_entries: Vec<MirrorEntry> = entries
        .into_iter()
        .filter(|e| e.state == Tri::Zero)
        .collect();

    if hold_entries.is_empty() {
        println!("No hold entries to revisit.");
        println!("Use 'mirror add' to add new entries with state 'hold'.");
        return Ok(());
    }

    println!("=== Revisit Hold Entries ===");
    println!("Found {} hold entries:", hold_entries.len());
    println!();

    for (i, entry) in hold_entries.iter().enumerate() {
        println!("{}. [HOLD] {}", i + 1, entry.input);
        if let Some(reason) = &entry.reason {
            println!("   Reason: {}", reason);
        }
        if !entry.tags.is_empty() {
            println!("   Tags: {}", entry.tags.join(", "));
        }
        println!("   ID: {}", entry.id);
        println!();
    }

    Ok(())
}
