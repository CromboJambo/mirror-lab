use chrono::Utc;
use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::decay::DecayConfig;

/// Distillation configuration for vacuum sealing.
#[derive(Debug, Clone)]
pub struct DistillationConfig {
    pub entropy_prune_threshold: f64,
    pub contradiction_max_depth: usize,
    pub checkpoint_interval_seconds: u64,
    pub provenance_id: String,
    pub set_at: i64,
    pub reason: String,
    pub source: String,
}

impl Default for DistillationConfig {
    fn default() -> Self {
        Self {
            entropy_prune_threshold: 0.3,
            contradiction_max_depth: 5,
            checkpoint_interval_seconds: 3600,
            provenance_id: Uuid::new_v4().to_string(),
            set_at: Utc::now().timestamp(),
            reason: "default distillation thresholds".to_string(),
            source: "mirror-log".to_string(),
        }
    }
}

impl DistillationConfig {
    pub fn with_entropy_prune_threshold(mut self, threshold: f64) -> Self {
        self.entropy_prune_threshold = threshold;
        self.provenance_id = Uuid::new_v4().to_string();
        self.set_at = Utc::now().timestamp();
        self
    }

    pub fn with_contradiction_max_depth(mut self, depth: usize) -> Self {
        self.contradiction_max_depth = depth;
        self.provenance_id = Uuid::new_v4().to_string();
        self.set_at = Utc::now().timestamp();
        self
    }

    pub fn with_checkpoint_interval(mut self, interval: u64) -> Self {
        self.checkpoint_interval_seconds = interval;
        self.provenance_id = Uuid::new_v4().to_string();
        self.set_at = Utc::now().timestamp();
        self
    }
}

/// Distillation result summary.
#[derive(Debug, Clone)]
pub struct DistillationResult {
    pub events_pruned: usize,
    pub contradictions_found: usize,
    pub shadowed_events: usize,
    pub checkpoint_id: String,
    pub timestamp: i64,
}

/// Entropy pruning: remove events from shadow_state whose decay score falls below the threshold.
pub fn entropy_prune(
    conn: &Connection,
    config: &DistillationConfig,
) -> Result<usize, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT event_id FROM shadow_state WHERE decay_score <= ?1")?;

    let rows = stmt.query_map(params![config.entropy_prune_threshold], |row| {
        Ok(row.get::<_, String>(0))
    })?;

    let mut ids = Vec::new();
    for row in rows {
        let id: String = row??;
        ids.push(id);
    }

    if ids.is_empty() {
        return Ok(0);
    }

    let mut pruned = 0;
    let tx = conn.unchecked_transaction()?;

    for id in ids {
        tx.execute("DELETE FROM shadow_state WHERE event_id = ?1", params![id])?;
        tx.execute("DELETE FROM decay WHERE event_id = ?1", params![id])?;
        tx.execute("DELETE FROM event_tags WHERE event_id = ?1", params![id])?;
        tx.execute(
            "DELETE FROM event_links WHERE from_event_id = ?1 OR to_event_id = ?1",
            params![id],
        )?;
        pruned += 1;
    }

    tx.commit()?;
    Ok(pruned)
}

/// Contradiction extraction: find event pairs with contradictory relations.
pub fn extract_contradictions(
    conn: &Connection,
    max_depth: usize,
) -> Result<usize, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT el1.from_event_id, el1.to_event_id, el1.relation, el2.from_event_id, el2.to_event_id, el2.relation
         FROM event_links el1
         JOIN event_links el2 ON el1.to_event_id = el2.from_event_id
         WHERE el1.relation = 'contradicts' AND el2.relation = 'supports'
         LIMIT ?1",
    )?;

    let rows = stmt.query_map(params![max_depth], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;

    let count = rows.count();
    Ok(count)
}

/// Context checkpointing: create a snapshot entry in the checkpoint table.
pub fn checkpoint(conn: &Connection) -> Result<String, rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS checkpoints (
            id TEXT PRIMARY KEY,
            event_count INTEGER NOT NULL,
            chunk_count INTEGER NOT NULL,
            shadow_count INTEGER NOT NULL,
            timestamp INTEGER NOT NULL DEFAULT (unixepoch()),
            snapshot_hash TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_checkpoints_time ON checkpoints(timestamp DESC);",
    )?;

    let event_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM active_events", [], |row| row.get(0))?;

    let chunk_count: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;

    let shadow_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM shadow_state", [], |row| row.get(0))?;

    let checkpoint_id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO checkpoints (id, event_count, chunk_count, shadow_count, timestamp)
         VALUES (?1, ?2, ?3, ?4, unixepoch())",
        params![checkpoint_id, event_count, chunk_count, shadow_count],
    )?;

    Ok(checkpoint_id)
}

/// Memory re-indexing: rebuild indexes for decay and shadow_state tables.
pub fn re_index(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_decay_access ON decay(access_count DESC);
         CREATE INDEX IF NOT EXISTS idx_decay_pinned ON decay(pinned);
         CREATE INDEX IF NOT EXISTS idx_shadow_flagged ON shadow_state(flagged_at DESC);",
    )?;

    Ok(())
}

/// Full distillation: entropy pruning + contradiction extraction + checkpoint + re-index.
pub fn distill(
    conn: &Connection,
    distillation_config: &DistillationConfig,
) -> Result<DistillationResult, rusqlite::Error> {
    let pruned = entropy_prune(conn, distillation_config)?;
    let contradictions = extract_contradictions(conn, distillation_config.contradiction_max_depth)?;
    let shadowed: i64 = conn
        .query_row("SELECT COUNT(*) FROM shadow_state", [], |row| row.get(0))
        .unwrap_or(0);

    let checkpoint_id = checkpoint(conn)?;

    re_index(conn)?;

    Ok(DistillationResult {
        events_pruned: pruned,
        contradictions_found: contradictions,
        shadowed_events: shadowed as usize,
        checkpoint_id,
        timestamp: Utc::now().timestamp(),
    })
}

/// Periodic trigger: checks whether the checkpoint interval has elapsed and triggers distillation if so.
pub fn periodic_trigger(
    conn: &Connection,
    distillation_config: &DistillationConfig,
    _decay_config: &DecayConfig,
) -> Result<Option<DistillationResult>, rusqlite::Error> {
    let last_checkpoint: Option<i64> = conn
        .query_row(
            "SELECT timestamp FROM checkpoints ORDER BY timestamp DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    let elapsed = if let Some(last) = last_checkpoint {
        Utc::now().timestamp() - last
    } else {
        0
    };

    if elapsed >= distillation_config.checkpoint_interval_seconds as i64 {
        Ok(Some(distill(conn, distillation_config)?))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_distillation_creates_checkpoint() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("mirror.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                source TEXT NOT NULL,
                content TEXT NOT NULL,
                ingested_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE IF NOT EXISTS shadow_state (
                event_id TEXT PRIMARY KEY,
                decay_score REAL NOT NULL,
                flagged_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE IF NOT EXISTS decay (
                event_id TEXT PRIMARY KEY,
                access_count INTEGER NOT NULL DEFAULT 0,
                last_accessed INTEGER NOT NULL,
                pinned BOOLEAN NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                event_id TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                start_offset INTEGER NOT NULL,
                end_offset INTEGER NOT NULL,
                timestamp INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS event_tags (
                id TEXT PRIMARY KEY,
                event_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE IF NOT EXISTS event_links (
                id TEXT PRIMARY KEY,
                from_event_id TEXT NOT NULL,
                to_event_id TEXT NOT NULL,
                relation TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE VIEW IF NOT EXISTS active_events AS
                SELECT * FROM events WHERE NOT EXISTS (
                    SELECT 1 FROM shadow_state s WHERE s.event_id = events.id
                );",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO events (id, timestamp, source, content) VALUES ('e1', 1000, 'test', 'hello')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO shadow_state (event_id, decay_score, flagged_at) VALUES ('e2', 0.2, 2000)",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO decay (event_id, access_count, last_accessed, pinned) VALUES ('e2', 0, 2000, 0)",
            [],
        ).unwrap();

        let distillation_config = DistillationConfig::default();
        let _decay_config = DecayConfig::default();

        let result = distill(&conn, &distillation_config).unwrap();

        assert!(result.checkpoint_id.len() > 0);
        assert_eq!(result.events_pruned, 1);
        assert_eq!(result.shadowed_events, 0);
    }

    #[test]
    fn test_checkpoint_table_created() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("mirror.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                source TEXT NOT NULL,
                content TEXT NOT NULL,
                ingested_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE IF NOT EXISTS shadow_state (
                event_id TEXT PRIMARY KEY,
                decay_score REAL NOT NULL,
                flagged_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE IF NOT EXISTS decay (
                event_id TEXT PRIMARY KEY,
                access_count INTEGER NOT NULL DEFAULT 0,
                last_accessed INTEGER NOT NULL,
                pinned BOOLEAN NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                event_id TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                start_offset INTEGER NOT NULL,
                end_offset INTEGER NOT NULL,
                timestamp INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS event_tags (
                id TEXT PRIMARY KEY,
                event_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE IF NOT EXISTS event_links (
                id TEXT PRIMARY KEY,
                from_event_id TEXT NOT NULL,
                to_event_id TEXT NOT NULL,
                relation TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE VIEW IF NOT EXISTS active_events AS
                SELECT * FROM events WHERE NOT EXISTS (
                    SELECT 1 FROM shadow_state s WHERE s.event_id = events.id
                );",
        )
        .unwrap();

        let checkpoint_id = checkpoint(&conn).unwrap();
        assert!(checkpoint_id.len() > 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM checkpoints", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_re_index_creates_indexes() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("mirror.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                source TEXT NOT NULL,
                content TEXT NOT NULL,
                ingested_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE IF NOT EXISTS shadow_state (
                event_id TEXT PRIMARY KEY,
                decay_score REAL NOT NULL,
                flagged_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE IF NOT EXISTS decay (
                event_id TEXT PRIMARY KEY,
                access_count INTEGER NOT NULL DEFAULT 0,
                last_accessed INTEGER NOT NULL,
                pinned BOOLEAN NOT NULL DEFAULT 0
            );
            CREATE VIEW IF NOT EXISTS active_events AS
                SELECT * FROM events WHERE NOT EXISTS (
                    SELECT 1 FROM shadow_state s WHERE s.event_id = events.id
                );",
        )
        .unwrap();

        re_index(&conn).unwrap();

        let sql: Option<String> = conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'index' AND name LIKE 'idx_decay%' OR name LIKE 'idx_shadow%' OR name LIKE 'idx_attention%'",
            [],
            |row| row.get(0),
        ).ok();

        assert!(sql.is_some());
    }

    #[test]
    fn test_periodic_trigger_returns_none_when_interval_not_elapsed() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("mirror.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                source TEXT NOT NULL,
                content TEXT NOT NULL,
                ingested_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE IF NOT EXISTS shadow_state (
                event_id TEXT PRIMARY KEY,
                decay_score REAL NOT NULL,
                flagged_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE IF NOT EXISTS decay (
                event_id TEXT PRIMARY KEY,
                access_count INTEGER NOT NULL DEFAULT 0,
                last_accessed INTEGER NOT NULL,
                pinned BOOLEAN NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                event_id TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                start_offset INTEGER NOT NULL,
                end_offset INTEGER NOT NULL,
                timestamp INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                event_count INTEGER NOT NULL,
                chunk_count INTEGER NOT NULL,
                shadow_count INTEGER NOT NULL,
                timestamp INTEGER NOT NULL DEFAULT (unixepoch()),
                snapshot_hash TEXT
            );
            CREATE VIEW IF NOT EXISTS active_events AS
                SELECT * FROM events WHERE NOT EXISTS (
                    SELECT 1 FROM shadow_state s WHERE s.event_id = events.id
                );",
        )
        .unwrap();

        let _checkpoint_id = checkpoint(&conn).unwrap();

        let distillation_config = DistillationConfig::default();
        let decay_config = DecayConfig::default();

        let result = periodic_trigger(&conn, &distillation_config, &decay_config).unwrap();

        assert!(result.is_none());
    }
}
