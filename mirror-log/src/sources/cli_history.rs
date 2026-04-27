use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};

/// Supported shell history locations
const BASH_HISTORY: &str = "~/.bash_history";
const ZSH_HISTORY: &str = "~/.zsh_history";
const NUSHELL_HISTORY: &str = "~/.config/nushell/history.db";

/// CLI History source for mirror-log
pub struct CliHistorySource;

impl CliHistorySource {
    /// Import shell history from supported shells
    pub fn import(conn: &Connection, dry_run: bool) -> Result<usize, String> {
        let mut imported = 0;

        // Try bash history
        if let Some(count) = Self::import_bash_history(conn, dry_run)? {
            imported += count;
        }

        // Try zsh history
        if let Some(count) = Self::import_zsh_history(conn, dry_run)? {
            imported += count;
        }

        // Try nushell history (sqlite)
        if let Some(count) = Self::import_nushell_history(conn, dry_run)? {
            imported += count;
        }

        Ok(imported)
    }

    /// Import bash history
    fn import_bash_history(conn: &Connection, dry_run: bool) -> Result<Option<usize>, String> {
        let path = Self::expand_path(BASH_HISTORY);

        if !path.exists() {
            return Ok(None);
        }

        let file = File::open(&path).map_err(|e| format!("Failed to open bash history: {}", e))?;
        let reader = BufReader::new(file);

        let mut dedup: HashMap<String, i64> = HashMap::new();

        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Calculate hash for deduplication
            let hash = Self::content_hash(trimmed);

            // Check if already exists
            if let Ok(true) = check_duplicate(conn, &hash) {
                continue;
            }

            // Store timestamp for deduplication
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            dedup.insert(hash, ts);
        }

        if dedup.is_empty() {
            return Ok(None);
        }

        // Batch insert
        let mut batch: Vec<(String, String, String)> = Vec::new();

        for (hash, ts) in dedup {
            let content = Self::get_content_by_hash(&path, ts).unwrap_or_else(|_| String::new());
            if !content.is_empty() {
                batch.push((hash, content, "cli-history".to_string()));
            }
        }

        if dry_run {
            println!("Would import {} bash history entries", batch.len());
            return Ok(Some(batch.len()));
        }

        let imported_count = batch.len();
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

        for (hash, content, source) in batch {
            let id = uuid::Uuid::new_v4().to_string();
            let now = Self::unix_now_secs();

            tx.execute(
                "INSERT INTO events (id, timestamp, source, content, meta, ingested_at, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    now,
                    source,
                    content,
                    Option::<String>::None,
                    now,
                    hash
                ],
            ).map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;

        Ok(Some(imported_count))
    }

    /// Import zsh history
    fn import_zsh_history(conn: &Connection, dry_run: bool) -> Result<Option<usize>, String> {
        let path = Self::expand_path(ZSH_HISTORY);

        if !path.exists() {
            return Ok(None);
        }

        let file = File::open(&path).map_err(|e| format!("Failed to open zsh history: {}", e))?;
        let reader = BufReader::new(file);

        let mut dedup: HashMap<String, i64> = HashMap::new();

        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let hash = Self::content_hash(trimmed);
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            dedup.insert(hash, ts);
        }

        if dedup.is_empty() {
            return Ok(None);
        }

        let mut batch: Vec<(String, String, String)> = Vec::new();

        for (hash, ts) in dedup {
            let content = Self::get_content_by_hash(&path, ts).unwrap_or_else(|_| String::new());
            if !content.is_empty() {
                batch.push((hash, content, "cli-history".to_string()));
            }
        }

        if dry_run {
            println!("Would import {} zsh history entries", batch.len());
            return Ok(Some(batch.len()));
        }

        let imported_count = batch.len();
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

        for (hash, content, source) in batch {
            let id = uuid::Uuid::new_v4().to_string();
            let now = Self::unix_now_secs();

            tx.execute(
                "INSERT INTO events (id, timestamp, source, content, meta, ingested_at, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    now,
                    source,
                    content,
                    Option::<String>::None,
                    now,
                    hash
                ],
            ).map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;

        Ok(Some(imported_count))
    }

    /// Import nushell history (sqlite)
    fn import_nushell_history(conn: &Connection, dry_run: bool) -> Result<Option<usize>, String> {
        let path = Self::expand_path(NUSHELL_HISTORY);

        if !path.exists() {
            return Ok(None);
        }

        // Open nushell history as sqlite
        let nushell_conn = Connection::open(&path)
            .map_err(|e| format!("Failed to open nushell history: {}", e))?;

        let mut dedup: HashMap<String, i64> = HashMap::new();

        // Query nushell history table
        let mut stmt = nushell_conn
            .prepare("SELECT value, created_at FROM history")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                let value: String = row.get(0)?;
                let created_at: i64 = row.get(1)?;
                Ok((value, created_at))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (value, created_at) = row.map_err(|e| e.to_string())?;
            let trimmed = value.trim();

            if trimmed.is_empty() {
                continue;
            }

            let hash = Self::content_hash(trimmed);
            dedup.insert(hash, created_at);
        }

        if dedup.is_empty() {
            return Ok(None);
        }

        let mut batch: Vec<(String, String, String)> = Vec::new();

        for (hash, ts) in dedup {
            let content = Self::get_content_by_hash(&path, ts).unwrap_or_else(|_| String::new());
            if !content.is_empty() {
                batch.push((hash, content, "cli-history".to_string()));
            }
        }

        if dry_run {
            println!("Would import {} nushell history entries", batch.len());
            return Ok(Some(batch.len()));
        }

        let imported_count = batch.len();
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

        for (hash, content, source) in batch {
            let id = uuid::Uuid::new_v4().to_string();
            let now = Self::unix_now_secs();

            tx.execute(
                "INSERT INTO events (id, timestamp, source, content, meta, ingested_at, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    now,
                    source,
                    content,
                    Option::<String>::None,
                    now,
                    hash
                ],
            ).map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;

        Ok(Some(imported_count))
    }

    /// Get content by timestamp from history file
    fn get_content_by_hash(_path: &PathBuf, _ts: i64) -> Result<String, String> {
        // TODO: Implement content retrieval by timestamp for bash/zsh history files
        // These files don't have per-line timestamps, so this requires parsing
        // For now, we skip content retrieval since we're using content_hash for dedup
        Ok(String::new())
    }

    /// Calculate SHA256 hash of content
    fn content_hash(content: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Expand ~ to home directory
    fn expand_path(path: &str) -> PathBuf {
        if path.starts_with('~')
            && let Ok(home) = std::env::var("HOME")
        {
            return PathBuf::from(home).join(path.strip_prefix('~').unwrap());
        }
        PathBuf::from(path)
    }

    /// Get current Unix timestamp
    fn unix_now_secs() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}

/// Check if content already exists in events table
fn check_duplicate(conn: &Connection, content_hash: &str) -> Result<bool, String> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM events WHERE content_hash = ?1)",
            [content_hash],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    Ok(exists)
}
