-- a-hole database schema
-- The observer log - structured tabular data for Nushell queries

-- Config change log
CREATE TABLE IF NOT EXISTS config_changes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    tool TEXT NOT NULL,
    file_path TEXT NOT NULL,
    old_value TEXT,
    new_value TEXT,
    change_type TEXT NOT NULL,
    outcome TEXT NOT NULL,
    user_context TEXT,
    metadata TEXT
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_config_changes_timestamp ON config_changes(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_config_changes_tool ON config_changes(tool);
CREATE INDEX IF NOT EXISTS idx_config_changes_outcome ON config_changes(outcome);
CREATE INDEX IF NOT EXISTS idx_config_changes_file_path ON config_changes(file_path);

-- Knowledge patterns table
CREATE TABLE IF NOT EXISTS knowledge_patterns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tool TEXT NOT NULL,
    file_path TEXT NOT NULL,
    pattern_type TEXT NOT NULL,
    pattern_data TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_seen TEXT NOT NULL
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_knowledge_patterns_tool ON knowledge_patterns(tool);
CREATE INDEX IF NOT EXISTS idx_knowledge_patterns_type ON knowledge_patterns(pattern_type);

-- Mod repository (for Layer 3)
CREATE TABLE IF NOT EXISTS mods (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    tool TEXT NOT NULL,
    author TEXT,
    description TEXT,
    safe_keys TEXT NOT NULL,
    delta TEXT NOT NULL,
    tags TEXT,
    downloads INTEGER DEFAULT 0,
    endorsements INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    is_flagged BOOLEAN DEFAULT FALSE,
    flag_reason TEXT
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_mods_tool ON mods(tool);
CREATE INDEX IF NOT EXISTS idx_mods_name ON mods(name);

-- Settings table
CREATE TABLE IF NOT EXISTS settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,
    value TEXT NOT NULL
);
