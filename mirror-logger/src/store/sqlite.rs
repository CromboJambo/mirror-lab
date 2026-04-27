use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, Result as SqliteResult, params};

use crate::entry::MirrorEntry;
use crate::tri::Tri;

/// SQLite storage layer for mirror-log
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    /// Create a new SQLite store
    pub fn new(db_path: &Path) -> SqliteResult<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS mirror_entries (
                id INTEGER PRIMARY KEY,
                input TEXT NOT NULL,
                state INTEGER NOT NULL,
                reason TEXT,
                timestamp INTEGER NOT NULL,
                tags TEXT,
                parent INTEGER
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_state ON mirror_entries(state)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_timestamp ON mirror_entries(timestamp)",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn insert(&self, entry: &MirrorEntry) -> SqliteResult<u64> {
        let conn = self.conn.lock().unwrap();
        let tags_json = serde_json::to_string(&entry.tags).unwrap_or_else(|_| "[]".to_string());

        conn.execute(
            "INSERT INTO mirror_entries (input, state, reason, timestamp, tags, parent)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &entry.input,
                entry.state.value(),
                entry.reason.as_deref(),
                entry.timestamp,
                tags_json,
                entry.parent,
            ],
        )?;

        Ok(conn.last_insert_rowid() as u64)
    }

    pub fn get_all(&self) -> SqliteResult<Vec<MirrorEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, input, state, reason, timestamp, tags, parent
             FROM mirror_entries
             ORDER BY timestamp DESC",
        )?;

        let entries = stmt
            .query_map([], Self::map_entry)?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(entries)
    }

    pub fn get_by_state(&self, state: Tri) -> SqliteResult<Vec<MirrorEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, input, state, reason, timestamp, tags, parent
             FROM mirror_entries
             WHERE state = ?1
             ORDER BY timestamp DESC",
        )?;

        let entries = stmt
            .query_map(params![state.value()], Self::map_entry)?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(entries)
    }

    pub fn get_holds(&self) -> SqliteResult<Vec<MirrorEntry>> {
        self.get_by_state(Tri::Zero)
    }

    pub fn get_by_id(&self, id: u64) -> SqliteResult<Option<MirrorEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, input, state, reason, timestamp, tags, parent
             FROM mirror_entries
             WHERE id = ?1",
        )?;

        match stmt.query_row(params![id], Self::map_entry) {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    }

    #[allow(dead_code)]
    pub fn count(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM mirror_entries", [], |row| row.get(0))
    }

    #[allow(dead_code)]
    pub fn stats(&self) -> SqliteResult<(i64, i64, i64)> {
        let conn = self.conn.lock().unwrap();
        let total = conn.query_row("SELECT COUNT(*) FROM mirror_entries", [], |row| row.get(0))?;
        let holds = conn.query_row(
            "SELECT COUNT(*) FROM mirror_entries WHERE state = ?1",
            params![Tri::Zero.value()],
            |row| row.get(0),
        )?;
        let resolved = conn.query_row(
            "SELECT COUNT(*) FROM mirror_entries WHERE state != ?1",
            params![Tri::Zero.value()],
            |row| row.get(0),
        )?;

        Ok((total, holds, resolved))
    }

    fn map_entry(row: &rusqlite::Row<'_>) -> SqliteResult<MirrorEntry> {
        let state_value: i32 = row.get(2)?;
        let tags_json: String = row.get(5)?;
        let tags = serde_json::from_str(&tags_json).unwrap_or_default();

        Ok(MirrorEntry {
            id: row.get(0)?,
            input: row.get(1)?,
            state: match state_value {
                -1 => Tri::Neg,
                1 => Tri::Pos,
                _ => Tri::Zero,
            },
            reason: row.get(3)?,
            timestamp: row.get(4)?,
            tags,
            parent: row.get(6)?,
        })
    }
}
