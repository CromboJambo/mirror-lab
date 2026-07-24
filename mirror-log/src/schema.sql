-- Mirror-Log Core Schema
-- This file is included by mirror-log/src/db.rs during database initialization.

CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL DEFAULT 'unknown',
    event_type TEXT NOT NULL,
    content TEXT,
    timestamp INTEGER NOT NULL,
    hash TEXT UNIQUE,
    created_at INTEGER DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_events_source ON events(source);
CREATE INDEX IF NOT EXISTS idx_events_hash ON events(hash);
