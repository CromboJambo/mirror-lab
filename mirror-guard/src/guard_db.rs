use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

use crate::concierge::{InterruptedLogEntry, PendingQueueEntry};
use crate::types::*;

#[derive(Debug, Error)]
pub enum GuardDbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("database path not available: {0}")]
    PathError(String),

    #[error("schema initialization failed: {0}")]
    SchemaError(String),
}

/// Manages the guard database connection and schema initialization.
/// Uses a separate DB file from mirror-log to maintain detection/action separation.
pub struct GuardDb {
    conn: Arc<Mutex<Connection>>,
}

impl GuardDb {
    /// Open or create the guard database at the given path.
    /// Default path is `guard.db` in the same directory as the mirror database.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, GuardDbError> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    /// Derive guard DB path from mirror DB path by replacing filename with `guard.db`.
    pub fn from_mirror_path(mirror_path: impl AsRef<Path>) -> PathBuf {
        let p = mirror_path.as_ref();
        let mut guard_path = p.parent().unwrap_or(Path::new(".")).to_path_buf();
        guard_path.push("guard.db");
        guard_path
    }

    /// Open guard DB co-located with mirror DB.
    pub fn co_located(mirror_path: impl AsRef<Path>) -> Result<Self, GuardDbError> {
        let guard_path = Self::from_mirror_path(mirror_path);
        Self::open(guard_path)
    }

    fn init_schema(&self) -> Result<(), GuardDbError> {
        let schema = include_str!("schema.sql");
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(schema)?;
        Ok(())
    }

    /// Get a guarded reference to the connection.
    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    // -- Anneal config helpers --

    pub fn load_anneal_config(&self) -> Result<AnnealConfig, GuardDbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM anneal_config")?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut config = AnnealConfig::default();
        for row in rows {
            let (key, value) = row?;
            match key.as_str() {
                "decay_rate" => config.decay_rate = value.parse().unwrap_or(config.decay_rate),
                "reinforce_threshold" => {
                    config.reinforce_threshold = value.parse().unwrap_or(config.reinforce_threshold)
                }
                "anneal_interval_seconds" => {
                    config.anneal_interval_seconds =
                        value.parse().unwrap_or(config.anneal_interval_seconds)
                }
                "max_anneal_passes" => {
                    config.max_anneal_passes = value.parse().unwrap_or(config.max_anneal_passes)
                }
                "confidence_floor" => {
                    config.confidence_floor = value.parse().unwrap_or(config.confidence_floor)
                }
                "auto_anneal_enabled" => {
                    config.auto_anneal_enabled = value.parse().unwrap_or(config.auto_anneal_enabled)
                }
                _ => {}
            }
        }

        Ok(config)
    }

    pub fn save_anneal_config(&self, config: &AnnealConfig) -> Result<(), GuardDbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO anneal_config (key, value) VALUES ('decay_rate', ?1)",
            params![config.decay_rate.to_string()],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO anneal_config (key, value) VALUES ('reinforce_threshold', ?1)",
            params![config.reinforce_threshold.to_string()],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO anneal_config (key, value) VALUES ('anneal_interval_seconds', ?1)",
            params![config.anneal_interval_seconds.to_string()],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO anneal_config (key, value) VALUES ('max_anneal_passes', ?1)",
            params![config.max_anneal_passes.to_string()],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO anneal_config (key, value) VALUES ('confidence_floor', ?1)",
            params![config.confidence_floor.to_string()],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO anneal_config (key, value) VALUES ('auto_anneal_enabled', ?1)",
            params![config.auto_anneal_enabled.to_string()],
        )?;
        Ok(())
    }

    pub fn persist_pending_queue_entry(
        &self,
        entry: &PendingQueueEntry,
    ) -> Result<(), GuardDbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO pending_queue (id, gate_result_id, action_type, command, args, trust_layer, confidence, source_event_id, queued_at, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                entry.id,
                entry.gate_result_id,
                entry.action_type,
                entry.command,
                serde_json::to_string(&entry.args)
                    .map_err(|e| GuardDbError::SchemaError(e.to_string()))?,
                entry.trust_layer,
                entry.confidence.to_string(),
                entry.source_event_id,
                entry.queued_at,
                entry.reason,
            ],
        )?;
        Ok(())
    }

    pub fn persist_interrupted_log_entry(
        &self,
        entry: &InterruptedLogEntry,
    ) -> Result<(), GuardDbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO interrupted_log (id, gate_result_id, action_type, command, args, trust_layer, source_event_id, reason, logged_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.id,
                entry.gate_result_id,
                entry.action_type,
                entry.command,
                serde_json::to_string(&entry.args)
                    .map_err(|e| GuardDbError::SchemaError(e.to_string()))?,
                entry.trust_layer,
                entry.source_event_id,
                entry.reason,
                entry.logged_at,
            ],
        )?;
        Ok(())
    }

    pub fn read_pending_queue(&self) -> Result<Vec<PendingQueueEntry>, GuardDbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, gate_result_id, action_type, command, args, trust_layer, confidence, source_event_id, queued_at, reason FROM pending_queue ORDER BY queued_at DESC",
        )?;
        let entries: Vec<PendingQueueEntry> = stmt
            .query_map([], |row| {
                let args_str: String = row.get(4)?;
                let args: Vec<String> = serde_json::from_str(&args_str)
                    .map_err(|e| GuardDbError::SchemaError(e.to_string()))
                    .map_err(|_e| rusqlite::Error::QueryReturnedNoRows)?;
                Ok(PendingQueueEntry {
                    id: row.get(0)?,
                    gate_result_id: row.get(1)?,
                    action_type: row.get(2)?,
                    command: row.get(3)?,
                    args,
                    trust_layer: row.get(5)?,
                    confidence: row
                        .get::<_, String>(6)?
                        .parse::<f64>()
                        .map_err(|_e| GuardDbError::SchemaError(_e.to_string()))
                        .map_err(|_e| rusqlite::Error::QueryReturnedNoRows)?,
                    source_event_id: row.get(7)?,
                    queued_at: row.get(8)?,
                    reason: row.get(9)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(entries)
    }

    pub fn read_interrupted_log(&self) -> Result<Vec<InterruptedLogEntry>, GuardDbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, gate_result_id, action_type, command, args, trust_layer, source_event_id, reason, logged_at FROM interrupted_log ORDER BY logged_at DESC",
        )?;
        let entries: Vec<InterruptedLogEntry> = stmt
            .query_map([], |row| {
                let args_str: String = row.get(4)?;
                let args: Vec<String> = serde_json::from_str(&args_str)
                    .map_err(|e| GuardDbError::SchemaError(e.to_string()))
                    .map_err(|_e| rusqlite::Error::QueryReturnedNoRows)?;
                Ok(InterruptedLogEntry {
                    id: row.get(0)?,
                    gate_result_id: row.get(1)?,
                    action_type: row.get(2)?,
                    command: row.get(3)?,
                    args,
                    trust_layer: row.get(5)?,
                    source_event_id: row.get(6)?,
                    reason: row.get(7)?,
                    logged_at: row.get(8)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_open_and_init_schema() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("guard.db");
        let db = GuardDb::open(&db_path).unwrap();
        assert!(db_path.exists());

        let conn = db.conn();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM trust_layers", [], |r| r.get(0))
            .unwrap();
        assert!(count >= 4);
    }

    #[test]
    fn test_from_mirror_path() {
        let path = PathBuf::from("/some/dir/mirror.db");
        let guard_path = GuardDb::from_mirror_path(&path);
        assert_eq!(guard_path, PathBuf::from("/some/dir/guard.db"));
    }

    #[test]
    fn test_load_default_anneal_config() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("guard.db");
        let db = GuardDb::open(&db_path).unwrap();

        let config = db.load_anneal_config().unwrap();
        assert_eq!(config.decay_rate, 0.02);
        assert_eq!(config.auto_anneal_enabled, true);
    }
}
