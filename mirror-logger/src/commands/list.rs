use crate::store::{JsonlStore, SqliteStore};
use crate::tri::Tri;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Filter by state: pass, fail, or hold (default: show all)
    #[arg(short, long)]
    pub state: Option<String>,
}

pub fn list_entries(args: ListArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Get paths
    let jsonl_path = PathBuf::from("data/mirror.log.jsonl");
    let db_path = PathBuf::from("data/mirror.db");

    // Initialize stores
    let _jsonl_store = JsonlStore::new(&jsonl_path)?;
    let sqlite_store = SqliteStore::new(&db_path)?;

    // Get entries
    let entries = if let Some(state_str) = args.state {
        let state = Tri::parse_str(&state_str).ok_or(format!(
            "Invalid state: {}. Must be 'pass', 'fail', or 'hold'",
            state_str
        ))?;
        sqlite_store.get_by_state(state)?
    } else {
        sqlite_store.get_all()?
    };

    // Display entries
    if entries.is_empty() {
        println!("No entries found.");
        return Ok(());
    }

    println!("Found {} entries:", entries.len());
    println!();

    for (i, entry) in entries.iter().enumerate() {
        println!("{}. [{}] {}", i + 1, entry.state.to_str(), entry.input);
        if let Some(reason) = &entry.reason {
            println!("   Reason: {}", reason);
        }
        if !entry.tags.is_empty() {
            println!("   Tags: {}", entry.tags.join(", "));
        }
        if let Some(parent) = entry.parent {
            println!("   Parent: {}", parent);
        }
        println!("   Timestamp: {}", entry.timestamp);
        println!();
    }

    Ok(())
}
