use rusqlite::{Connection, Result, params};
use std::time::UNIX_EPOCH;
use uuid::Uuid;

use chrono::Utc;

#[derive(Debug, Clone)]
pub struct DecayConfig {
    pub decay_threshold_days: i64,
    pub access_count_threshold: i64,
    pub provenance_id: String,
    pub set_at: i64,
    pub reason: String,
    pub source: String,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            decay_threshold_days: DECAY_THRESHOLD_DAYS,
            access_count_threshold: ACCESS_COUNT_THRESHOLD,
            provenance_id: Uuid::new_v4().to_string(),
            set_at: Utc::now().timestamp(),
            reason: "default decay thresholds".to_string(),
            source: "mirror-log".to_string(),
        }
    }
}

impl DecayConfig {
    pub fn with_decay_threshold(mut self, days: i64) -> Self {
        self.decay_threshold_days = days;
        self.provenance_id = Uuid::new_v4().to_string();
        self.set_at = Utc::now().timestamp();
        self
    }

    pub fn with_access_count_threshold(mut self, count: i64) -> Self {
        self.access_count_threshold = count;
        self.provenance_id = Uuid::new_v4().to_string();
        self.set_at = Utc::now().timestamp();
        self
    }
}

const DECAY_THRESHOLD_DAYS: i64 = 30;
const ACCESS_COUNT_THRESHOLD: i64 = 10;

/// Initialize decay-related tables (Note: Tables are now defined in schema.sql)
pub fn init_decay_tables(_conn: &Connection) -> Result<()> {
    Ok(())
}

/// Increment access count for an event and update last_accessed timestamp
pub fn track_access(conn: &Connection, event_id: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO decay (event_id, last_accessed)
         VALUES (?1, unixepoch())
         ON CONFLICT(event_id) DO UPDATE SET
            access_count = decay.access_count + 1,
            last_accessed = unixepoch(),
            pinned = decay.pinned",
        [event_id],
    )?;
    Ok(())
}

/// Get decay score for an event: access_count / days_since_logged
pub fn get_decay_score(conn: &Connection, event_id: &str) -> Result<f64> {
    let score: Option<f64> = conn.query_row(
        "SELECT
            CASE
                WHEN last_accessed = 0 THEN 0
                WHEN CAST((unixepoch() - last_accessed) / 86400 AS INTEGER) < 1 THEN CAST(access_count AS REAL)
                ELSE CAST(access_count AS REAL) /
                     CAST((unixepoch() - last_accessed) / 86400 AS REAL)
            END
         FROM decay
         WHERE event_id = ?1",
        [event_id],
        |row| row.get(0),
    )?;
    Ok(score.unwrap_or(0.0))
}

/// Check if an event is flagged for decay (below threshold)
pub fn is_flagged(conn: &Connection, event_id: &str, config: &DecayConfig) -> Result<bool> {
    let flagged: bool = conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM decay
            WHERE event_id = ?1
            AND access_count < ?2
            AND (unixepoch() - last_accessed) > ?3 * 86400
            AND pinned = 0
        )",
        params![
            event_id,
            config.access_count_threshold,
            config.decay_threshold_days
        ],
        |row| row.get(0),
    )?;
    Ok(flagged)
}

/// Get all events that meet decay criteria
pub fn get_flagged_events(conn: &Connection, config: &DecayConfig) -> Result<Vec<String>> {
    let mut ids = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT e.id FROM events e
          JOIN decay d ON e.id = d.event_id
          LEFT JOIN shadow_state s ON e.id = s.event_id
          WHERE d.access_count < ?
          AND (unixepoch() - d.last_accessed) > ? * 86400
          AND d.pinned = 0
          AND s.event_id IS NULL
          ORDER BY d.last_accessed ASC",
    )?;

    let rows = stmt.query_map(
        params![config.access_count_threshold, config.decay_threshold_days],
        |row| row.get(0),
    )?;

    for row in rows {
        ids.push(row?);
    }

    Ok(ids)
}

/// Move flagged events to the shadow state so they no longer appear in normal queries.
pub fn move_to_shadow(conn: &Connection, config: &DecayConfig) -> Result<usize> {
    let flagged_ids = get_flagged_events(conn, config)?;

    if flagged_ids.is_empty() {
        return Ok(0);
    }

    // Get decay scores for flagged events
    let mut scores: Vec<(String, f64)> = Vec::new();
    for id in &flagged_ids {
        if let Ok(score) = get_decay_score(conn, id) {
            scores.push((id.clone(), score));
        }
    }

    let tx = conn.unchecked_transaction()?;
    let mut moved = 0;
    for (id, score) in scores {
        let now = UNIX_EPOCH
            .elapsed()
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        tx.execute(
            "INSERT INTO shadow_state (event_id, decay_score, flagged_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(event_id) DO UPDATE SET
                decay_score = excluded.decay_score,
                flagged_at = excluded.flagged_at",
            params![id, score, now],
        )?;

        moved += 1;
    }

    tx.commit()?;
    Ok(moved)
}

/// Get all shadowed events
pub fn get_shadow_events(conn: &Connection) -> Result<Vec<ShadowEvent>> {
    let mut events = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT e.id, e.timestamp, e.source, e.content, e.meta, e.ingested_at, e.content_hash, s.decay_score, s.flagged_at
         FROM shadow_state s
         JOIN events e ON e.id = s.event_id
         ORDER BY flagged_at DESC"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ShadowEvent {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            source: row.get(2)?,
            content: row.get(3)?,
            meta: row.get(4)?,
            ingested_at: row.get(5)?,
            content_hash: row.get(6)?,
            decay_score: row.get(7)?,
            flagged_at: row.get(8)?,
        })
    })?;

    for row in rows {
        events.push(row?);
    }

    Ok(events)
}

/// Pin an event (immune to decay)
pub fn pin_event(conn: &Connection, event_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE decay SET pinned = 1 WHERE event_id = ?1",
        [event_id],
    )?;
    Ok(())
}

/// Unpin an event
pub fn unpin_event(conn: &Connection, event_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE decay SET pinned = 0 WHERE event_id = ?1",
        [event_id],
    )?;
    Ok(())
}

/// Restore an event from shadow back to main table
pub fn restore_from_shadow(conn: &Connection, event_id: &str) -> Result<()> {
    conn.execute("DELETE FROM shadow_state WHERE event_id = ?1", [event_id])?;

    // Reset decay tracking
    conn.execute(
        "INSERT INTO decay (event_id, access_count, last_accessed, pinned)
         VALUES (?1, 0, unixepoch(), 0)
         ON CONFLICT(event_id) DO UPDATE SET
            access_count = 0,
            last_accessed = unixepoch(),
            pinned = 0",
        [event_id],
    )?;

    Ok(())
}

/// Get decay statistics
pub fn get_decay_stats(conn: &Connection, config: &DecayConfig) -> Result<DecayStats> {
    let total_events: i64 = conn.query_row("SELECT COUNT(*) FROM decay", [], |row| row.get(0))?;

    let total_access: i64 =
        conn.query_row("SELECT SUM(access_count) FROM decay", [], |row| row.get(0))?;

    let pinned_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM decay WHERE pinned = 1", [], |row| {
            row.get(0)
        })?;

    let avg_access: f64 = if total_events > 0 {
        total_access as f64 / total_events as f64
    } else {
        0.0
    };

    let flagged_count: i64 = conn.query_row(
        "SELECT COUNT(*)
          FROM decay d
          LEFT JOIN shadow_state s ON d.event_id = s.event_id
          WHERE d.access_count < ?1
          AND (unixepoch() - d.last_accessed) > ?2 * 86400
          AND d.pinned = 0
          AND s.event_id IS NULL",
        params![config.access_count_threshold, config.decay_threshold_days],
        |row| row.get(0),
    )?;

    Ok(DecayStats {
        total_events,
        total_access,
        pinned_count,
        avg_access,
        flagged_count,
    })
}

#[derive(Debug, Clone)]
pub struct ShadowEvent {
    pub id: String,
    pub timestamp: i64,
    pub source: String,
    pub content: String,
    pub meta: Option<String>,
    pub ingested_at: i64,
    pub content_hash: Option<String>,
    pub decay_score: f64,
    pub flagged_at: i64,
}

#[derive(Debug, Clone)]
pub struct DecayStats {
    pub total_events: i64,
    pub total_access: i64,
    pub pinned_count: i64,
    pub avg_access: f64,
    pub flagged_count: i64,
}
