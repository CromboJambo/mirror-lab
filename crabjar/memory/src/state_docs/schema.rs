use crate::error::Result;
use rusqlite::Connection;

/// Single source of truth for state-docs DDL.
/// Migrations are append-only — never modify existing steps.
const STATE_DOCS_MIGRATIONS: &[&str] = &[
    // v1 — baseline
    "CREATE TABLE IF NOT EXISTS doc_metadata (
        id          INTEGER PRIMARY KEY,
        doc_name    TEXT NOT NULL UNIQUE,
        description TEXT NOT NULL DEFAULT '',
        last_modified TEXT NOT NULL DEFAULT (datetime('now')),
        line_count  INTEGER NOT NULL DEFAULT 0,
        checksum    TEXT NOT NULL DEFAULT ''
    )",
    "CREATE TABLE IF NOT EXISTS sections (
        id          INTEGER PRIMARY KEY,
        doc_id      INTEGER NOT NULL REFERENCES doc_metadata(id),
        level       INTEGER NOT NULL,
        title       TEXT NOT NULL,
        start_line  INTEGER NOT NULL,
        end_line    INTEGER NOT NULL,
        parent_id   INTEGER REFERENCES sections(id),
        content_hash TEXT NOT NULL DEFAULT ''
    )",
    "CREATE TABLE IF NOT EXISTS tables (
        id          INTEGER PRIMARY KEY,
        doc_id      INTEGER NOT NULL REFERENCES doc_metadata(id),
        section_id  INTEGER REFERENCES sections(id),
        start_line  INTEGER NOT NULL,
        end_line    INTEGER NOT NULL,
        headers     TEXT NOT NULL DEFAULT '[]',
        rows        TEXT NOT NULL DEFAULT '[]'
    )",
    "CREATE TABLE IF NOT EXISTS code_blocks (
        id          INTEGER PRIMARY KEY,
        doc_id      INTEGER NOT NULL REFERENCES doc_metadata(id),
        section_id  INTEGER REFERENCES sections(id),
        start_line  INTEGER NOT NULL,
        end_line    INTEGER NOT NULL,
        language    TEXT NOT NULL DEFAULT '',
        content     TEXT NOT NULL DEFAULT '',
        content_hash TEXT NOT NULL DEFAULT ''
    )",
    "CREATE TABLE IF NOT EXISTS confidence (
        id          INTEGER PRIMARY KEY,
        doc_id      INTEGER NOT NULL REFERENCES doc_metadata(id),
        what_captured TEXT NOT NULL DEFAULT '',
        what_missed TEXT NOT NULL DEFAULT '',
        assumptions TEXT NOT NULL DEFAULT '[]',
        blind_spots TEXT NOT NULL DEFAULT '[]',
        stale_after TEXT NOT NULL DEFAULT ''
    )",
    "CREATE TABLE IF NOT EXISTS annotations (
        id          INTEGER PRIMARY KEY,
        doc_id      INTEGER NOT NULL REFERENCES doc_metadata(id),
        section_id  INTEGER REFERENCES sections(id),
        line        INTEGER NOT NULL,
        kind        TEXT NOT NULL,
        status      TEXT NOT NULL,
        author      TEXT NOT NULL,
        message     TEXT NOT NULL,
        created_at  TEXT NOT NULL DEFAULT (datetime('now'))
    )",
    // Indexes for fast query
    "CREATE INDEX IF NOT EXISTS idx_sections_doc_level ON sections(doc_id, level)",
    "CREATE INDEX IF NOT EXISTS idx_sections_parent ON sections(parent_id)",
    "CREATE INDEX IF NOT EXISTS idx_tables_doc ON tables(doc_id)",
    "CREATE INDEX IF NOT EXISTS idx_code_blocks_doc ON code_blocks(doc_id)",
    "CREATE INDEX IF NOT EXISTS idx_annotations_doc_line ON annotations(doc_id, line)",
    "CREATE INDEX IF NOT EXISTS idx_confidence_doc ON confidence(doc_id)",
    "CREATE INDEX IF NOT EXISTS idx_doc_metadata_name ON doc_metadata(doc_name)",
];

/// Apply state-docs schema migrations to a connection.
pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

    for ddl in STATE_DOCS_MIGRATIONS {
        conn.execute_batch(ddl)?;
    }

    Ok(())
}
