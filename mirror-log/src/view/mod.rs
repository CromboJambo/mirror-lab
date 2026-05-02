use chrono::{TimeZone, Utc};
use rusqlite::{Connection, Result};

#[derive(Debug)]
pub struct Event {
    pub id: String,
    pub timestamp: i64,
    pub source: String,
    pub content: String,
    pub meta: Option<String>,
    pub ingested_at: i64,
    pub content_hash: Option<String>,
}

impl Event {
    pub fn format_time(&self) -> String {
        let dt = match Utc.timestamp_opt(self.timestamp, 0) {
            chrono::LocalResult::Single(dt) => dt,
            chrono::LocalResult::Ambiguous(dt, _) => dt,
            chrono::LocalResult::None => Utc::now(),
        };
        dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }

    pub fn format_ingested_at(&self) -> String {
        let dt = match Utc.timestamp_opt(self.ingested_at, 0) {
            chrono::LocalResult::Single(dt) => dt,
            chrono::LocalResult::Ambiguous(dt, _) => dt,
            chrono::LocalResult::None => Utc::now(),
        };
        dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }

    pub fn preview_content(&self, max_chars: usize) -> String {
        let total_chars = self.content.chars().count();
        if total_chars <= max_chars {
            self.content.clone()
        } else {
            let preview: String = self.content.chars().take(max_chars).collect();
            format!(
                "{}...\n\n[Content truncated: {} of {} chars shown]",
                preview, max_chars, total_chars
            )
        }
    }
}

pub fn recent(conn: &Connection, limit: i64) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, source, content, meta, ingested_at, content_hash
         FROM active_events
         ORDER BY timestamp DESC
         LIMIT ?1",
    )?;

    let rows = stmt.query_map([limit], |row| {
        Ok(Event {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            source: row.get(2)?,
            content: row.get(3)?,
            meta: row.get(4)?,
            ingested_at: row.get(5)?,
            content_hash: row.get(6)?,
        })
    })?;

    rows.collect()
}

pub fn search(conn: &Connection, term: &str) -> Result<Vec<Event>> {
    let like = format!("%{}%", term);

    let mut stmt = conn.prepare(
        "SELECT id, timestamp, source, content, meta, ingested_at, content_hash
         FROM active_events
         WHERE content LIKE ?1
         ORDER BY timestamp DESC",
    )?;

    let rows = stmt.query_map([like], |row| {
        Ok(Event {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            source: row.get(2)?,
            content: row.get(3)?,
            meta: row.get(4)?,
            ingested_at: row.get(5)?,
            content_hash: row.get(6)?,
        })
    })?;

    rows.collect()
}

pub fn by_source(conn: &Connection, source: &str, limit: Option<i64>) -> Result<Vec<Event>> {
    let query = if let Some(lim) = limit {
        format!(
            "SELECT id, timestamp, source, content, meta, ingested_at, content_hash
             FROM active_events
             WHERE source = ?1
             ORDER BY timestamp DESC
             LIMIT {}",
            lim
        )
    } else {
        "SELECT id, timestamp, source, content, meta, ingested_at, content_hash
         FROM active_events
         WHERE source = ?1
         ORDER BY timestamp DESC"
            .to_string()
    };

    let mut stmt = conn.prepare(&query)?;

    let rows = stmt.query_map([source], |row| {
        Ok(Event {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            source: row.get(2)?,
            content: row.get(3)?,
            meta: row.get(4)?,
            ingested_at: row.get(5)?,
            content_hash: row.get(6)?,
        })
    })?;

    rows.collect()
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Event> {
    conn.query_row(
        "SELECT id, timestamp, source, content, meta, ingested_at, content_hash
         FROM events
         WHERE id = ?1",
        [id],
        |row| {
            Ok(Event {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                source: row.get(2)?,
                content: row.get(3)?,
                meta: row.get(4)?,
                ingested_at: row.get(5)?,
                content_hash: row.get(6)?,
            })
        },
    )
}

/// Get events ingested within a time range
pub fn by_ingestion_time(
    conn: &Connection,
    start: i64,
    end: i64,
    limit: Option<i64>,
) -> Result<Vec<Event>> {
    let query = if let Some(lim) = limit {
        format!(
            "SELECT id, timestamp, source, content, meta, ingested_at, content_hash
             FROM active_events
             WHERE ingested_at BETWEEN ?1 AND ?2
             ORDER BY ingested_at DESC
             LIMIT {}",
            lim
        )
    } else {
        "SELECT id, timestamp, source, content, meta, ingested_at, content_hash
         FROM active_events
         WHERE ingested_at BETWEEN ?1 AND ?2
         ORDER BY ingested_at DESC"
            .to_string()
    };

    let mut stmt = conn.prepare(&query)?;

    let rows = stmt.query_map([start, end], |row| {
        Ok(Event {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            source: row.get(2)?,
            content: row.get(3)?,
            meta: row.get(4)?,
            ingested_at: row.get(5)?,
            content_hash: row.get(6)?,
        })
    })?;

    rows.collect()
}

/// Get deduplication statistics
pub fn dedup_stats(conn: &Connection) -> Result<(i64, i64)> {
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM active_events", [], |row| row.get(0))?;
    let unique: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT content_hash) FROM active_events",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok((total, unique))
}

/// Filter events by content hash to find duplicates
pub fn find_duplicates(conn: &Connection, content_hash: &str) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, source, content, meta, ingested_at, content_hash
         FROM active_events
         WHERE content_hash = ?1
         ORDER BY ingested_at ASC",
    )?;

    let rows = stmt.query_map([content_hash], |row| {
        Ok(Event {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            source: row.get(2)?,
            content: row.get(3)?,
            meta: row.get(4)?,
            ingested_at: row.get(5)?,
            content_hash: row.get(6)?,
        })
    })?;

    rows.collect()
}
