use serde::{Deserialize, Serialize};
use std::fmt;

/// Confidence score for a memory node, 0.0 (unknown) to 1.0 (certain).
/// Decays over time unless reinforced by successful outcomes.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct TrustScore(f64);

impl TrustScore {
    pub const fn new(score: f64) -> Self {
        Self(score.clamp(0.0, 1.0))
    }

    pub const fn get(&self) -> f64 {
        self.0
    }

    pub const fn is_zero(&self) -> bool {
        self.0 == 0.0
    }

    pub fn reinforce(&self, delta: f64) -> Self {
        Self::new(self.0 + delta)
    }

    pub fn decay(&self, rate: f64) -> Self {
        Self::new(self.0 - rate)
    }

    pub fn interpolate(&self, other: &Self, weight: f64) -> Self {
        let blended = self.0 * (1.0 - weight) + other.0 * weight;
        Self::new(blended)
    }
}

impl Default for TrustScore {
    fn default() -> Self {
        Self(0.5)
    }
}

impl fmt::Display for TrustScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3}", self.0)
    }
}

/// Kind of memory node
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Fact,
    Pattern,
    Rule,
    Reflection,
    Outcome,
    Residue,
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeKind::Fact => write!(f, "fact"),
            NodeKind::Pattern => write!(f, "pattern"),
            NodeKind::Rule => write!(f, "rule"),
            NodeKind::Reflection => write!(f, "reflection"),
            NodeKind::Outcome => write!(f, "outcome"),
            NodeKind::Residue => write!(f, "residue"),
        }
    }
}

/// A node in the memory graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryNode {
    pub id: String,
    pub kind: NodeKind,
    pub content: String,
    pub trust_layer: u32,
    pub confidence: TrustScore,
    pub created_at: i64,
    pub last_touched: i64,
    pub anneal_count: u32,
    pub metadata: Option<String>,
}

/// Relationship between memory nodes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeRelation {
    Supports,
    Contradicts,
    DerivedFrom,
    Anneals,
    DependsOn,
    EvidenceFor,
}

impl fmt::Display for EdgeRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EdgeRelation::Supports => write!(f, "supports"),
            EdgeRelation::Contradicts => write!(f, "contradicts"),
            EdgeRelation::DerivedFrom => write!(f, "derived_from"),
            EdgeRelation::Anneals => write!(f, "anneals"),
            EdgeRelation::DependsOn => write!(f, "depends_on"),
            EdgeRelation::EvidenceFor => write!(f, "evidence_for"),
        }
    }
}

/// A directed, weighted edge between memory nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEdge {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    pub relation: EdgeRelation,
    pub weight: f64,
    pub created_at: i64,
}

/// Configurable trust layer band
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustLayer {
    pub id: u32,
    name: String,
    pub min_confidence: f64,
    pub max_confidence: f64,
    pub auto_execute: bool,
    pub requires_review: bool,
    description: Option<String>,
}

impl TrustLayer {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn contains_score(&self, score: TrustScore) -> bool {
        score.get() >= self.min_confidence && score.get() < self.max_confidence
    }
}

/// Review action taken by human or automated reviewer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewAction {
    Approve,
    Reject,
    Modify,
    Escalate,
}

impl fmt::Display for ReviewAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReviewAction::Approve => write!(f, "approve"),
            ReviewAction::Reject => write!(f, "reject"),
            ReviewAction::Modify => write!(f, "modify"),
            ReviewAction::Escalate => write!(f, "escalate"),
        }
    }
}

/// Record of a human or automated review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRecord {
    pub id: String,
    pub node_id: String,
    pub reviewer: String,
    pub action: ReviewAction,
    pub old_confidence: Option<TrustScore>,
    pub new_confidence: Option<TrustScore>,
    pub old_trust_layer: Option<u32>,
    pub new_trust_layer: Option<u32>,
    pub notes: Option<String>,
    pub created_at: i64,
}

/// Status of an action request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionStatus {
    Pending,
    Approved,
    Denied,
    Executed,
    Interrupted,
}

impl fmt::Display for ActionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionStatus::Pending => write!(f, "pending"),
            ActionStatus::Approved => write!(f, "approved"),
            ActionStatus::Denied => write!(f, "denied"),
            ActionStatus::Executed => write!(f, "executed"),
            ActionStatus::Interrupted => write!(f, "interrupted"),
        }
    }
}

/// Request to perform an action, gated by trust layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub id: String,
    pub source_event_id: Option<String>,
    pub action_type: String,
    pub payload: String,
    pub trust_layer: u32,
    pub confidence: TrustScore,
    pub status: ActionStatus,
    pub gate_result: Option<String>,
    pub requested_at: i64,
    pub resolved_at: Option<i64>,
}

/// Outcome of an executed action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionOutcome {
    pub id: String,
    pub action_id: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub output_hash: Option<String>,
    pub residual: Option<String>,
    pub skill_residue: Option<String>,
    pub confidence_delta: f64,
    pub created_at: i64,
}

/// Annealing configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnealConfig {
    pub decay_rate: f64,
    pub reinforce_threshold: f64,
    pub anneal_interval_seconds: u64,
    pub max_anneal_passes: u32,
    pub confidence_floor: f64,
    pub auto_anneal_enabled: bool,
}

impl Default for AnnealConfig {
    fn default() -> Self {
        Self {
            decay_rate: 0.02,
            reinforce_threshold: 0.7,
            anneal_interval_seconds: 3600,
            max_anneal_passes: 10,
            confidence_floor: 0.05,
            auto_anneal_enabled: true,
        }
    }
}

/// Result of an annealing pass
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnealResult {
    pub nodes_processed: usize,
    pub nodes_upgraded: usize,
    pub nodes_downgraded: usize,
    pub nodes_decayed: usize,
    pub edges_pruned: usize,
    pub pass_number: u32,
    pub timestamp: i64,
}

/// Retrieval band filter for querying memory nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalBand {
    pub min_trust_layer: u32,
    pub max_trust_layer: u32,
    pub min_confidence: f64,
    pub kinds: Option<Vec<NodeKind>>,
    pub max_results: usize,
}

impl Default for RetrievalBand {
    fn default() -> Self {
        Self {
            min_trust_layer: 0,
            max_trust_layer: 3,
            min_confidence: 0.0,
            kinds: None,
            max_results: 100,
        }
    }
}

impl RetrievalBand {
    pub fn working_and_above() -> Self {
        Self {
            min_trust_layer: 2,
            max_trust_layer: 3,
            min_confidence: 0.5,
            kinds: None,
            max_results: 50,
        }
    }

    pub fn annealed_only() -> Self {
        Self {
            min_trust_layer: 3,
            max_trust_layer: 3,
            min_confidence: 0.8,
            kinds: None,
            max_results: 25,
        }
    }

    pub fn with_kinds(mut self, kinds: Vec<NodeKind>) -> Self {
        self.kinds = Some(kinds);
        self
    }
}
