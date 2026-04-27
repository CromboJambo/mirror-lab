use crate::error::Result;
use rusqlite::Connection;

/// Single source of truth for DDL.
/// Migrations are append-only — never modify existing steps.
const MIGRATIONS: &[&str] = &[
    // v1 — baseline
    "CREATE TABLE IF NOT EXISTS knowledge (
        id       INTEGER PRIMARY KEY,
        content  TEXT NOT NULL,
        kind     TEXT NOT NULL,
        tags     TEXT NOT NULL DEFAULT '[]',
        meta     TEXT NOT NULL DEFAULT '{}',
        weight   REAL NOT NULL DEFAULT 1.0,
        active   INTEGER NOT NULL DEFAULT 1,
        created  TEXT NOT NULL DEFAULT (datetime('now')),
        checksum TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS events (
        id        INTEGER PRIMARY KEY,
        kind      TEXT NOT NULL,
        target_id INTEGER,
        payload   TEXT,
        source    TEXT NOT NULL,
        ts        TEXT NOT NULL DEFAULT (datetime('now'))
    )",
    // Indexes that make the tag query fast
    "CREATE INDEX IF NOT EXISTS idx_knowledge_active  ON knowledge(active)",
    "CREATE INDEX IF NOT EXISTS idx_knowledge_weight  ON knowledge(weight DESC)",
    "CREATE INDEX IF NOT EXISTS idx_events_target     ON events(target_id)",
    "CREATE INDEX IF NOT EXISTS idx_events_ts         ON events(ts)",
    // Migration tracking table — must come after baseline
    "CREATE TABLE IF NOT EXISTS schema_versions (
        version   INTEGER PRIMARY KEY,
        applied   TEXT NOT NULL DEFAULT (datetime('now')),
        note      TEXT
    )",
];

pub fn migrate(conn: &Connection) -> Result<()> {
    // WAL mode — readers don't block writers
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_versions (
            version   INTEGER PRIMARY KEY,
            applied   TEXT NOT NULL DEFAULT (datetime('now')),
            note      TEXT
        )",
    )?;

    // Apply each migration idempotently
    for (i, ddl) in MIGRATIONS.iter().enumerate() {
        conn.execute_batch(ddl)?;

        // Record version if not already present
        let note = &ddl[..ddl.len().min(80)];
        conn.execute(
            "INSERT OR IGNORE INTO schema_versions (version, note)
             VALUES (?1, ?2)",
            rusqlite::params![i as i64, note],
        )?;

        // Note: We use a separate execute here because the version table might not exist yet.
    }

    Ok(())
}
