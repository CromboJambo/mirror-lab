-- mirror-guard Schema: Annealing knowledge distillation system
-- Separate DB from mirror-log to keep detection (mirror-log) and action gating (mirror-guard) layers distinct

-- ============================================================================
-- Memory Graph: nodes and edges representing knowledge structures
-- ============================================================================
CREATE TABLE IF NOT EXISTS memory_nodes (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('fact', 'pattern', 'rule', 'reflection', 'outcome', 'residue')),
    content TEXT NOT NULL,
    trust_layer INTEGER NOT NULL DEFAULT 0,
    confidence REAL NOT NULL DEFAULT 1.0 CHECK (confidence >= 0.0 AND confidence <= 1.0),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_touched INTEGER NOT NULL DEFAULT (unixepoch()),
    anneal_count INTEGER NOT NULL DEFAULT 0,
    metadata TEXT
);

CREATE INDEX IF NOT EXISTS idx_nodes_kind ON memory_nodes(kind);
CREATE INDEX IF NOT EXISTS idx_nodes_trust ON memory_nodes(trust_layer);
CREATE INDEX IF NOT EXISTS idx_nodes_confidence ON memory_nodes(confidence DESC);
CREATE INDEX IF NOT EXISTS idx_nodes_touched ON memory_nodes(last_touched DESC);

-- Edges between memory nodes (weighted, directed)
CREATE TABLE IF NOT EXISTS memory_edges (
    id TEXT PRIMARY KEY,
    from_id TEXT NOT NULL,
    to_id TEXT NOT NULL,
    relation TEXT NOT NULL CHECK (relation IN ('supports', 'contradicts', 'derived_from', 'anneals', 'depends_on', 'evidence_for')),
    weight REAL NOT NULL DEFAULT 1.0 CHECK (weight >= 0.0 AND weight <= 1.0),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (from_id) REFERENCES memory_nodes(id) ON DELETE CASCADE,
    FOREIGN KEY (to_id) REFERENCES memory_nodes(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_edges_from ON memory_edges(from_id);
CREATE INDEX IF NOT EXISTS idx_edges_to ON memory_edges(to_id);
CREATE INDEX IF NOT EXISTS idx_edges_relation ON memory_edges(relation);

-- ============================================================================
-- Trust Layers: configurable trust bands
-- ============================================================================
CREATE TABLE IF NOT EXISTS trust_layers (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    min_confidence REAL NOT NULL CHECK (min_confidence >= 0.0 AND min_confidence <= 1.0),
    max_confidence REAL NOT NULL CHECK (max_confidence >= 0.0 AND max_confidence <= 1.0),
    auto_execute BOOLEAN NOT NULL DEFAULT 0,
    requires_review BOOLEAN NOT NULL DEFAULT 0,
    description TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Seed default trust layers
INSERT OR IGNORE INTO trust_layers (id, name, min_confidence, max_confidence, auto_execute, requires_review, description) VALUES
    (0, 'raw',          0.0, 0.2, 0, 1, 'Unverified raw observations'),
    (1, 'observed',     0.2, 0.5, 0, 1, 'Observed but not yet confirmed'),
    (2, 'working',      0.5, 0.8, 0, 1, 'Working knowledge - requires review'),
    (3, 'annealed',     0.8, 1.0, 1, 0, 'Highly annealed, trusted knowledge');

-- ============================================================================
-- Review Records: human review history
-- ============================================================================
CREATE TABLE IF NOT EXISTS review_records (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    reviewer TEXT NOT NULL DEFAULT 'human',
    action TEXT NOT NULL CHECK (action IN ('approve', 'reject', 'modify', 'escalate')),
    old_confidence REAL,
    new_confidence REAL,
    old_trust_layer INTEGER,
    new_trust_layer INTEGER,
    notes TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (node_id) REFERENCES memory_nodes(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_reviews_node ON review_records(node_id);
CREATE INDEX IF NOT EXISTS idx_reviews_action ON review_records(action);
CREATE INDEX IF NOT EXISTS idx_reviews_time ON review_records(created_at DESC);

-- ============================================================================
-- Action Tracking: requests and outcomes
-- ============================================================================
CREATE TABLE IF NOT EXISTS action_requests (
    id TEXT PRIMARY KEY,
    source_event_id TEXT,
    source_node_id TEXT,
    action_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    trust_layer INTEGER NOT NULL,
    confidence REAL NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'denied', 'executed', 'interrupted')),
    gate_result TEXT,
    requested_at INTEGER NOT NULL DEFAULT (unixepoch()),
    resolved_at INTEGER,
    FOREIGN KEY (source_node_id) REFERENCES memory_nodes(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_actions_status ON action_requests(status);
CREATE INDEX IF NOT EXISTS idx_actions_trust ON action_requests(trust_layer);
CREATE INDEX IF NOT EXISTS idx_actions_time ON action_requests(requested_at DESC);

CREATE TABLE IF NOT EXISTS action_outcomes (
    id TEXT PRIMARY KEY,
    action_id TEXT NOT NULL UNIQUE,
    success BOOLEAN NOT NULL,
    exit_code INTEGER,
    output_hash TEXT,
    residual TEXT,
    skill_residue TEXT,
    confidence_delta REAL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (action_id) REFERENCES action_requests(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_outcomes_action ON action_outcomes(action_id);
CREATE INDEX IF NOT EXISTS idx_outcomes_success ON action_outcomes(success);

-- ============================================================================
-- Annealing Configuration
-- ============================================================================
CREATE TABLE IF NOT EXISTS anneal_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Default annealing configuration
INSERT OR IGNORE INTO anneal_config (key, value) VALUES
    ('decay_rate', '0.02'),
    ('reinforce_threshold', '0.7'),
    ('anneal_interval_seconds', '3600'),
    ('max_anneal_passes', '10'),
    ('confidence_floor', '0.05'),
    ('auto_anneal_enabled', '1');

-- ============================================================================
-- Views
-- ============================================================================
CREATE VIEW IF NOT EXISTS node_trust_view AS
SELECT
    n.id,
    n.kind,
    n.content,
    n.confidence,
    n.trust_layer,
    tl.name AS trust_name,
    n.anneal_count,
    n.last_touched,
    n.created_at
FROM memory_nodes n
JOIN trust_layers tl ON n.trust_layer = tl.id;

CREATE INDEX IF NOT EXISTS idx_actions_node ON action_requests(source_node_id);

CREATE VIEW IF NOT EXISTS pending_actions AS
SELECT ar.*, n.confidence AS node_confidence
FROM action_requests ar
LEFT JOIN memory_nodes n ON ar.source_node_id = n.id
WHERE ar.status = 'pending';
