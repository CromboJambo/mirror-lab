CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    tags TEXT NOT NULL -- CSV or JSON
);

CREATE TABLE IF NOT EXISTS reflections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_event_ids TEXT NOT NULL, -- CSV or JSON of event IDs
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    tags TEXT NOT NULL
);
