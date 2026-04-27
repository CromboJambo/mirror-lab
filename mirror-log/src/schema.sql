-- mirror-log Schema with Iteration Tracking
-- This schema supports high-volume ingestion, deduplication, rich enrichment, and iterative learning cycles

-- ============================================================================
-- Core Events Table
-- ============================================================================
CREATE TABLE IF NOT EXISTS events (
    id TEXT PRIMARY KEY,
    timestamp INTEGER NOT NULL,           -- Event creation timestamp (UTC seconds)
    source TEXT NOT NULL CHECK (length(source) > 0), -- Source identifier (e.g., "cli", "stdin", "file")
    content TEXT NOT NULL,                 -- Raw event content
    meta TEXT,                             -- Optional JSON metadata
    ingested_at INTEGER NOT NULL DEFAULT (unixepoch()),  -- Ingestion timestamp
    content_hash TEXT CHECK (content_hash IS NULL OR length(content_hash) = 64) -- SHA256 hash for deduplication
);

-- ============================================================================
-- Performance Indexes
-- ============================================================================
-- Fast retrieval by timestamp (descending for recent events)
CREATE INDEX IF NOT EXISTS idx_events_ts ON events(timestamp DESC);

-- Composite index for source + timestamp queries
CREATE INDEX IF NOT EXISTS idx_events_source_ts ON events(source, timestamp DESC);

-- Deduplication lookup index (NULL-safe, duplicates allowed)
CREATE INDEX IF NOT EXISTS idx_events_hash ON events(content_hash) WHERE content_hash IS NOT NULL;

-- ============================================================================
-- Chunked Content Table (for large events)
-- ============================================================================
CREATE TABLE IF NOT EXISTS chunks (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_chunks_event ON chunks(event_id);
CREATE INDEX IF NOT EXISTS idx_chunks_time ON chunks(timestamp DESC);

-- ============================================================================
-- Enrichment Tables
-- ============================================================================
-- Event tags: structured metadata
CREATE TABLE IF NOT EXISTS event_tags (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_tags_event ON event_tags(event_id);
CREATE INDEX IF NOT EXISTS idx_tags_tag ON event_tags(tag);

-- Event links: semantic relationships between events
CREATE TABLE IF NOT EXISTS event_links (
    id TEXT PRIMARY KEY,
    from_event_id TEXT NOT NULL,
    to_event_id TEXT NOT NULL,
    relation TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (from_event_id) REFERENCES events(id) ON DELETE CASCADE,
    FOREIGN KEY (to_event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_links_from ON event_links(from_event_id);
CREATE INDEX IF NOT EXISTS idx_links_to ON event_links(to_event_id);

-- Event embeddings: vector embeddings for AI search
CREATE TABLE IF NOT EXISTS event_embeddings (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    embedding BLOB NOT NULL,  -- Binary vector representation
    model_name TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (event_id)REFERENCES events(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_embeddings_event ON event_embeddings(event_id);
CREATE INDEX IF NOT EXISTS idx_embeddings_model ON event_embeddings(model_name);

-- Enrichment jobs: track background enrichment processes
CREATE TABLE IF NOT EXISTS enrichment_jobs (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    job_type TEXT NOT NULL,  -- e.g., "tag", "link", "embed"
    status TEXT NOT NULL DEFAULT 'pending',  -- 'pending', 'running', 'completed', 'failed'
    attempts INTEGER NOT NULL DEFAULT 0,
    result TEXT,  -- JSON result or error message
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_jobs_event ON enrichment_jobs(event_id);
CREATE INDEX IF NOT EXISTS idx_jobs_status ON enrichment_jobs(status);
CREATE INDEX IF NOT EXISTS idx_jobs_type ON enrichment_jobs(job_type);

-- ============================================================================
-- Iteration Tracking Tables
-- ============================================================================

-- Iteration passes: track how many times an event has been iterated
CREATE TABLE IF NOT EXISTS iteration_passes (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    iteration_number INTEGER NOT NULL CHECK (iteration_number > 0),
    pass_type TEXT NOT NULL,  -- 'exposure', 'reflection', 're-encoding', 'application'
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_passes_event ON iteration_passes(event_id);
CREATE INDEX IF NOT EXISTS idx_passes_number ON iteration_passes(event_id, iteration_number);
CREATE INDEX IF NOT EXISTS idx_passes_type ON iteration_passes(pass_type);
CREATE INDEX IF NOT EXISTS idx_passes_time ON iteration_passes(created_at DESC);

-- Iteration insight metrics: track insight quality per iteration (dy/dx concept)
CREATE TABLE IF NOT EXISTS iteration_insight (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    iteration_number INTEGER NOT NULL,
    insight_score INTEGER NOT NULL CHECK (insight_score >= 0),  -- 0-100 scale
    insight_delta REAL NOT NULL CHECK (insight_delta <= 0),  -- negative value (improvement from previous)
    feedback_quality TEXT NOT NULL,  -- 'poor', 'fair', 'good', 'excellent'
    semantic_change TEXT,  -- JSON describing semantic changes
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_insight_event ON iteration_insight(event_id);
CREATE INDEX IF NOT EXISTS idx_insight_number ON iteration_insight(event_id, iteration_number);
CREATE INDEX IF NOT EXISTS idx_insight_score ON iteration_insight(insight_score DESC);
CREATE INDEX IF NOT EXISTS idx_insight_delta ON iteration_insight(insight_delta);

-- Iteration feedback: detailed feedback for each iteration
CREATE TABLE IF NOT EXISTS iteration_feedback (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    iteration_number INTEGER NOT NULL,
    hint TEXT NOT NULL,  -- The hint given at this iteration
    user_response TEXT,  -- User's response to the hint
    response_quality TEXT NOT NULL,  -- 'wrong', 'partial', 'correct', 'excellent'
    response_time INTEGER,  -- Time taken to respond (in seconds)
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_feedback_event ON iteration_feedback(event_id);
CREATE INDEX IF NOT EXISTS idx_feedback_number ON iteration_feedback(event_id, iteration_number);
CREATE INDEX IF NOT EXISTS idx_feedback_quality ON iteration_feedback(response_quality);

-- Iteration thresholds: configurable thresholds for when to stop iterating
CREATE TABLE IF NOT EXISTS iteration_thresholds (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    pass_type TEXT NOT NULL,
    max_iterations INTEGER NOT NULL CHECK (max_iterations > 0),
    insight_threshold INTEGER NOT NULL CHECK (insight_threshold >= 0),  -- Stop when insight score drops below this
    delta_threshold REAL NOT NULL CHECK (delta_threshold <= 0),  -- Stop when insight improvement drops below this
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_thresholds_event ON iteration_thresholds(event_id);
CREATE INDEX IF NOT EXISTS idx_thresholds_type ON iteration_thresholds(pass_type);

-- Iteration status: current state of iteration for each event
CREATE TABLE IF NOT EXISTS iteration_status (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    current_iteration INTEGER NOT NULL DEFAULT 0,
    current_pass_type TEXT,  -- 'exposure', 'reflection', 're-encoding', 'application'
    last_insight_score INTEGER,
    last_insight_delta REAL,
    is_complete BOOLEAN NOT NULL DEFAULT 0,
    completion_reason TEXT,  -- 'max_iterations', 'insight_threshold', 'delta_threshold', 'manual'
    completed_at INTEGER,
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_status_event ON iteration_status(event_id);
CREATE INDEX IF NOT EXISTS idx_status_complete ON iteration_status(is_complete);

-- Iteration statistics: aggregated statistics per event
CREATE TABLE IF NOT EXISTS iteration_stats (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    total_iterations INTEGER NOT NULL DEFAULT 0,
    total_passes INTEGER NOT NULL DEFAULT 0,
    average_insight_score REAL,
    max_insight_score REAL,
    min_insight_score REAL,
    total_improvement REAL,
    avg_improvement REAL,
    completion_time INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_stats_event ON iteration_stats(event_id);

-- ============================================================================
-- Decay & Attention Tracking
-- ============================================================================
CREATE TABLE IF NOT EXISTS decay (
    event_id TEXT PRIMARY KEY,
    access_count INTEGER NOT NULL DEFAULT 0,
    last_accessed INTEGER NOT NULL,
    pinned BOOLEAN NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS shadow_state (
    event_id TEXT PRIMARY KEY,
    decay_score REAL NOT NULL,
    flagged_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

-- ============================================================================
-- Views
-- ============================================================================
CREATE VIEW IF NOT EXISTS active_events AS
SELECT * FROM events
WHERE NOT EXISTS (
    SELECT 1 FROM shadow_state s WHERE s.event_id = events.id
);

-- ============================================================================
-- SQLite Performance Pragmas
-- ============================================================================
-- Recommended pragmas for production use
-- Note: These should be set at application startup
-- PRAGMA journal_mode = WAL;          -- Write-Ahead Logging for better concurrency
-- PRAGMA synchronous = NORMAL;        -- Balance between safety and performance
-- PRAGMA temp_store = MEMORY;         -- Use memory for temporary tables
-- PRAGMA page_size = 4096;            -- Optimal page size for most systems
-- PRAGMA mmap_size = 30000000000;     -- Memory-mapped I/O for large databases
-- PRAGMA cache_size = -10000;         -- 10,000 pages (~40MB)
-- PRAGMA foreign_keys = ON;           -- Enforce referential integrity
