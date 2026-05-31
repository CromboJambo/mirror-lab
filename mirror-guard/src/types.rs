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
    pub(crate) name: String,
    pub min_confidence: f64,
    pub max_confidence: f64,
    pub auto_execute: bool,
    pub requires_review: bool,
    pub(crate) description: Option<String>,
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
    TrustApproved,
    Denied,
    Executed,
    Interrupted,
}

/// Status of an action outcome record
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutcomeStatus {
    Executed,
    ExecutedTrustUpdateFailed,
}

impl fmt::Display for ActionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionStatus::Pending => write!(f, "pending"),
            ActionStatus::TrustApproved => write!(f, "trust-approved"),
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
    /// Raw event ID from mirror-log — provenance of the triggering observation
    pub source_event_id: Option<String>,
    /// Memory node ID from mirror-guard — the derived knowledge authorizing this action
    pub source_node_id: Option<String>,
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
            max_trust_layer: 4,
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
            max_trust_layer: 4,
            min_confidence: 0.5,
            kinds: None,
            max_results: 50,
        }
    }

    pub fn annealed_only() -> Self {
        Self {
            min_trust_layer: 4,
            max_trust_layer: 4,
            min_confidence: 0.9,
            kinds: None,
            max_results: 25,
        }
    }

    pub fn with_kinds(mut self, kinds: Vec<NodeKind>) -> Self {
        self.kinds = Some(kinds);
        self
    }
}

/// Model inference provenance type for guard gate tracking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelInferenceKind {
    Prompt,
    ContextAugmented,
    SkillAugmented,
    EmergentSkill,
}

impl fmt::Display for ModelInferenceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelInferenceKind::Prompt => write!(f, "prompt"),
            ModelInferenceKind::ContextAugmented => write!(f, "context-augmented"),
            ModelInferenceKind::SkillAugmented => write!(f, "skill-augmented"),
            ModelInferenceKind::EmergentSkill => write!(f, "emergent-skill"),
        }
    }
}

/// Model inference request gated by trust layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInferenceRequest {
    pub id: String,
    pub provenance_id: String,
    pub model_name: String,
    pub weight_id: String,
    pub inference_kind: ModelInferenceKind,
    pub prompt: String,
    pub context: Vec<String>,
    pub skill_refs: Vec<String>,
    pub trust_layer: u32,
    pub confidence: TrustScore,
    pub status: ActionStatus,
    pub gate_result: Option<String>,
    pub requested_at: i64,
    pub resolved_at: Option<i64>,
}

/// Model inference outcome for confidence tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInferenceOutcome {
    pub id: String,
    pub inference_id: String,
    pub model_name: String,
    pub weight_id: String,
    pub output_hash: String,
    pub skill_residue: Option<String>,
    pub confidence_delta: f64,
    pub success: bool,
    pub created_at: i64,
}

/// PID trust record for per-process authorization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PidTrustRecord {
    pub pid: i32,
    pub trust_layer: u32,
    pub use_count: u64,
    pub last_use: i64,
    pub auto_grant: bool,
    pub decay_interval: i64,
    pub decay_rate: f64,
}

impl Default for PidTrustRecord {
    fn default() -> Self {
        Self {
            pid: 0,
            trust_layer: 0,
            use_count: 0,
            last_use: 0,
            auto_grant: false,
            decay_interval: 3600,
            decay_rate: 0.02,
        }
    }
}

/// Record of a guided revocation exit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokedLogEntry {
    pub id: i64,
    pub pid: i32,
    pub command: String,
    pub revoked_at: i64,
    pub reason: String,
    pub old_layer: u32,
    pub new_layer: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_score_new_clamps_to_one() {
        assert_eq!(TrustScore::new(1.5).get(), 1.0);
        assert_eq!(TrustScore::new(-0.5).get(), 0.0);
        assert_eq!(TrustScore::new(0.5).get(), 0.5);
    }

    #[test]
    fn trust_score_is_zero() {
        assert!(TrustScore::new(0.0).is_zero());
        assert!(!TrustScore::new(0.001).is_zero());
    }

    #[test]
    fn trust_score_default() {
        let score = TrustScore::default();
        assert_eq!(score.get(), 0.5);
    }

    #[test]
    fn trust_score_reinforce() {
        let base = TrustScore::new(0.5);
        let reinforced = base.reinforce(0.3);
        assert_eq!(reinforced.get(), 0.8);
    }

    #[test]
    fn trust_score_decay() {
        let base = TrustScore::new(0.5);
        let decayed = base.decay(0.2);
        assert_eq!(decayed.get(), 0.3);
    }

    #[test]
    fn trust_score_interpolate() {
        let a = TrustScore::new(0.0);
        let b = TrustScore::new(1.0);
        let blended = a.interpolate(&b, 0.5);
        assert_eq!(blended.get(), 0.5);
    }

    #[test]
    fn trust_score_display() {
        let score = TrustScore::new(0.1234);
        assert_eq!(format!("{}", score), "0.123");
    }

    #[test]
    fn node_kind_display_fact() {
        assert_eq!(format!("{}", NodeKind::Fact), "fact");
    }

    #[test]
    fn node_kind_display_pattern() {
        assert_eq!(format!("{}", NodeKind::Pattern), "pattern");
    }

    #[test]
    fn node_kind_display_rule() {
        assert_eq!(format!("{}", NodeKind::Rule), "rule");
    }

    #[test]
    fn node_kind_display_reflection() {
        assert_eq!(format!("{}", NodeKind::Reflection), "reflection");
    }

    #[test]
    fn node_kind_display_outcome() {
        assert_eq!(format!("{}", NodeKind::Outcome), "outcome");
    }

    #[test]
    fn node_kind_display_residue() {
        assert_eq!(format!("{}", NodeKind::Residue), "residue");
    }

    #[test]
    fn edge_relation_display_supports() {
        assert_eq!(format!("{}", EdgeRelation::Supports), "supports");
    }

    #[test]
    fn edge_relation_display_contradicts() {
        assert_eq!(format!("{}", EdgeRelation::Contradicts), "contradicts");
    }

    #[test]
    fn edge_relation_display_derived_from() {
        assert_eq!(format!("{}", EdgeRelation::DerivedFrom), "derived_from");
    }

    #[test]
    fn edge_relation_display_anneals() {
        assert_eq!(format!("{}", EdgeRelation::Anneals), "anneals");
    }

    #[test]
    fn edge_relation_display_depends_on() {
        assert_eq!(format!("{}", EdgeRelation::DependsOn), "depends_on");
    }

    #[test]
    fn edge_relation_display_evidence_for() {
        assert_eq!(format!("{}", EdgeRelation::EvidenceFor), "evidence_for");
    }

    #[test]
    fn trust_layer_contains_score() {
        let layer = TrustLayer {
            id: 1,
            name: "working".to_string(),
            min_confidence: 0.5,
            max_confidence: 0.9,
            auto_execute: false,
            requires_review: true,
            description: Some("test".to_string()),
        };
        assert!(layer.contains_score(TrustScore::new(0.6)));
        assert!(layer.contains_score(TrustScore::new(0.8)));
        assert!(!layer.contains_score(TrustScore::new(0.4)));
        assert!(!layer.contains_score(TrustScore::new(0.9)));
    }

    #[test]
    fn trust_layer_name() {
        let layer = TrustLayer {
            id: 1,
            name: "working".to_string(),
            min_confidence: 0.0,
            max_confidence: 1.0,
            auto_execute: false,
            requires_review: false,
            description: None,
        };
        assert_eq!(layer.name(), "working");
    }

    #[test]
    fn trust_layer_description() {
        let layer = TrustLayer {
            id: 1,
            name: "test".to_string(),
            min_confidence: 0.0,
            max_confidence: 1.0,
            auto_execute: false,
            requires_review: false,
            description: Some("desc".to_string()),
        };
        assert_eq!(layer.description(), Some("desc"));
    }

    #[test]
    fn review_action_display_approve() {
        assert_eq!(format!("{}", ReviewAction::Approve), "approve");
    }

    #[test]
    fn review_action_display_reject() {
        assert_eq!(format!("{}", ReviewAction::Reject), "reject");
    }

    #[test]
    fn review_action_display_modify() {
        assert_eq!(format!("{}", ReviewAction::Modify), "modify");
    }

    #[test]
    fn review_action_display_escalate() {
        assert_eq!(format!("{}", ReviewAction::Escalate), "escalate");
    }

    #[test]
    fn action_status_display_pending() {
        assert_eq!(format!("{}", ActionStatus::Pending), "pending");
    }

    #[test]
    fn action_status_display_trust_approved() {
        assert_eq!(format!("{}", ActionStatus::TrustApproved), "trust-approved");
    }

    #[test]
    fn action_status_display_denied() {
        assert_eq!(format!("{}", ActionStatus::Denied), "denied");
    }

    #[test]
    fn action_status_display_executed() {
        assert_eq!(format!("{}", ActionStatus::Executed), "executed");
    }

    #[test]
    fn action_status_display_interrupted() {
        assert_eq!(format!("{}", ActionStatus::Interrupted), "interrupted");
    }

    #[test]
    fn retrieval_band_default() {
        let band = RetrievalBand::default();
        assert_eq!(band.min_trust_layer, 0);
        assert_eq!(band.max_trust_layer, 4);
        assert_eq!(band.min_confidence, 0.0);
        assert!(band.kinds.is_none());
        assert_eq!(band.max_results, 100);
    }

    #[test]
    fn retrieval_band_working_and_above() {
        let band = RetrievalBand::working_and_above();
        assert_eq!(band.min_trust_layer, 2);
        assert_eq!(band.max_trust_layer, 4);
        assert_eq!(band.min_confidence, 0.5);
        assert_eq!(band.max_results, 50);
    }

    #[test]
    fn retrieval_band_annealed_only() {
        let band = RetrievalBand::annealed_only();
        assert_eq!(band.min_trust_layer, 4);
        assert_eq!(band.max_trust_layer, 4);
        assert_eq!(band.min_confidence, 0.9);
        assert_eq!(band.max_results, 25);
    }

    #[test]
    fn retrieval_band_with_kinds() {
        let band = RetrievalBand::default().with_kinds(vec![NodeKind::Fact, NodeKind::Rule]);
        assert!(band.kinds.is_some());
        assert_eq!(band.kinds.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn model_inference_kind_display_prompt() {
        assert_eq!(format!("{}", ModelInferenceKind::Prompt), "prompt");
    }

    #[test]
    fn model_inference_kind_display_context_augmented() {
        assert_eq!(
            format!("{}", ModelInferenceKind::ContextAugmented),
            "context-augmented"
        );
    }

    #[test]
    fn model_inference_kind_display_skill_augmented() {
        assert_eq!(
            format!("{}", ModelInferenceKind::SkillAugmented),
            "skill-augmented"
        );
    }

    #[test]
    fn model_inference_kind_display_emergent_skill() {
        assert_eq!(
            format!("{}", ModelInferenceKind::EmergentSkill),
            "emergent-skill"
        );
    }

    #[test]
    fn anneal_config_default() {
        let config = AnnealConfig::default();
        assert_eq!(config.decay_rate, 0.02);
        assert_eq!(config.reinforce_threshold, 0.7);
        assert_eq!(config.anneal_interval_seconds, 3600);
        assert_eq!(config.max_anneal_passes, 10);
        assert_eq!(config.confidence_floor, 0.05);
        assert!(config.auto_anneal_enabled);
    }

    #[test]
    fn memory_node_clone() {
        let node = MemoryNode {
            id: "test".to_string(),
            kind: NodeKind::Fact,
            content: "content".to_string(),
            trust_layer: 2,
            confidence: TrustScore::new(0.5),
            created_at: 0,
            last_touched: 0,
            anneal_count: 0,
            metadata: None,
        };
        let cloned = node.clone();
        assert_eq!(node.id, cloned.id);
        assert_eq!(node.kind, cloned.kind);
        assert_eq!(node.content, cloned.content);
    }

    #[test]
    fn memory_edge_clone() {
        let edge = MemoryEdge {
            id: "e1".to_string(),
            from_id: "n1".to_string(),
            to_id: "n2".to_string(),
            relation: EdgeRelation::Supports,
            weight: 1.0,
            created_at: 0,
        };
        let cloned = edge.clone();
        assert_eq!(edge.id, cloned.id);
        assert_eq!(edge.relation, cloned.relation);
    }

    #[test]
    fn action_status_equality() {
        assert_eq!(ActionStatus::Pending, ActionStatus::Pending);
        assert_ne!(ActionStatus::Pending, ActionStatus::Denied);
    }

    #[test]
    fn review_action_equality() {
        assert_eq!(ReviewAction::Approve, ReviewAction::Approve);
        assert_ne!(ReviewAction::Approve, ReviewAction::Reject);
    }

    #[test]
    fn node_kind_equality() {
        assert_eq!(NodeKind::Fact, NodeKind::Fact);
        assert_ne!(NodeKind::Fact, NodeKind::Rule);
    }

    #[test]
    fn edge_relation_equality() {
        assert_eq!(EdgeRelation::Supports, EdgeRelation::Supports);
        assert_ne!(EdgeRelation::Supports, EdgeRelation::Contradicts);
    }

    #[test]
    fn model_inference_kind_equality() {
        assert_eq!(ModelInferenceKind::Prompt, ModelInferenceKind::Prompt);
        assert_ne!(
            ModelInferenceKind::Prompt,
            ModelInferenceKind::ContextAugmented
        );
    }

    #[test]
    fn outcome_status_equality() {
        assert_eq!(OutcomeStatus::Executed, OutcomeStatus::Executed);
        assert_ne!(
            OutcomeStatus::Executed,
            OutcomeStatus::ExecutedTrustUpdateFailed
        );
    }
}
