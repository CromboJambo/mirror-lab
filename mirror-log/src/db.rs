use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, Result};

use crate::decay;

pub fn init_db(path: impl AsRef<Path>) -> Result<Connection> {
    let conn = Connection::open(path)?;

    // Performance optimization pragmas
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.pragma_update(None, "foreign_keys", 1)?;
    conn.busy_timeout(Duration::from_secs(5))?;

    conn.execute_batch(include_str!("schema.sql"))?;
    decay::init_decay_tables(&conn)?;
    Ok(conn)
}

pub fn db_info(conn: &Connection) -> Result<(i64, i64, i64)> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM events
         WHERE NOT EXISTS (
             SELECT 1 FROM shadow_state s WHERE s.event_id = events.id
         )",
        [],
        |row| row.get(0),
    )?;

    let oldest: i64 = conn
        .query_row(
            "SELECT MIN(timestamp)
             FROM events
             WHERE NOT EXISTS (
                 SELECT 1 FROM shadow_state s WHERE s.event_id = events.id
             )",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let newest: i64 = conn
        .query_row(
            "SELECT MAX(timestamp)
             FROM events
             WHERE NOT EXISTS (
                 SELECT 1 FROM shadow_state s WHERE s.event_id = events.id
             )",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok((count, oldest, newest))
}
