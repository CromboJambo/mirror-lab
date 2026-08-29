use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, Result};

use crate::decay;

/// The current schema version. Bump this when the shape of any table changes,
/// and add a corresponding step to `migrate_to` below. The version is stored in
/// SQLite's `user_version` pragma so it survives across connections and is
/// independent of any table contents.
pub const SCHEMA_VERSION: i32 = 1;

pub fn init_db(path: impl AsRef<Path>) -> Result<Connection> {
    let conn = Connection::open(path)?;

    // Performance optimization pragmas
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.pragma_update(None, "foreign_keys", 1)?;
    conn.busy_timeout(Duration::from_secs(5))?;

    // Apply the base schema (idempotent) and bring any older database up to
    // SCHEMA_VERSION. Safe on both fresh and existing databases.
    conn.execute_batch(include_str!("schema.sql"))?;
    migrate_to(&conn, SCHEMA_VERSION)?;

    decay::init_decay_tables(&conn)?;
    Ok(conn)
}

/// Reads the current schema version from the database. Returns 0 for a database
/// that has never been versioned (the legacy pre-versioning state).
pub fn current_schema_version(conn: &Connection) -> Result<i32> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(version)
}

/// Brings a database from its current version up to `target`, applying one
/// migration step per version. Each step runs inside a transaction so a partial
/// migration never leaves the database half-migrated.
///
/// Migration steps are keyed by the version they *produce*: the closure for
/// version N runs when the database is at version N-1.
fn migrate_to(conn: &Connection, target: i32) -> Result<()> {
    let current = current_schema_version(conn)?;
    if current >= target {
        return Ok(());
    }

    let tx = conn.unchecked_transaction()?;

    let mut version = current;
    while version < target {
        let next = version + 1;
        match next {
            1 => {
                // Version 0 -> 1: the base schema was already applied by the
                // idempotent schema.sql above, so nothing extra is needed here.
                // This step exists so the version is recorded and future
                // migrations have a stable baseline.
            }
            // Add future steps here, e.g.:
            // 2 => migrate_v1_to_v2(&tx)?,
            n => {
                // Unreachable in normal operation: `target` is always
                // SCHEMA_VERSION and every step up to it is handled above.
                // Reaching this means the version ladder and SCHEMA_VERSION
                // have drifted out of sync, which is a programming error.
                unreachable!("no migration path to schema version {n}");
            }
        }
        version = next;
    }

    tx.pragma_update(None, "user_version", version)?;
    tx.commit()?;
    Ok(())
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
