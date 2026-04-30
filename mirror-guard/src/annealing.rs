use rusqlite::params;
use tracing::{debug, info, warn};

use crate::guard_db::{GuardDb, GuardDbError};
use crate::memory::MemoryGraph;
use crate::trust::TrustManager;
use crate::types::*;

/// The annealing pipeline gradually distills knowledge through iterative passes.
///
/// Annealing works by:
/// 1. Decaying confidence of stale nodes (time-based decay)
/// 2. Reinforcing nodes with successful evidence
/// 3. Pruning low-weight edges
/// 4. Reassigning trust layers based on new confidence scores
///
/// The pipeline is declarative by default: it follows configured rules.
/// Selective fallback allows agent doubt to trigger manual review requests.
pub struct AnnealingPipeline<'a> {
    db: &'a GuardDb,
    graph: MemoryGraph<'a>,
    trust: TrustManager<'a>,
    config: AnnealConfig,
}

impl<'a> AnnealingPipeline<'a> {
    pub fn new(db: &'a GuardDb) -> Result<Self, GuardDbError> {
        let config = db.load_anneal_config()?;
        Ok(Self {
            db,
            graph: MemoryGraph::new(db),
            trust: TrustManager::new(db),
            config,
        })
    }

    /// Run a single annealing pass over all memory nodes.
    /// Returns a summary of changes made.
    pub fn run_pass(&self) -> Result<AnnealResult, GuardDbError> {
        info!("Starting annealing pass");
        let start = std::time::Instant::now();

        let mut result = AnnealResult {
            nodes_processed: 0,
            nodes_upgraded: 0,
            nodes_downgraded: 0,
            nodes_decayed: 0,
            edges_pruned: 0,
            pass_number: 0,
            timestamp: chrono::Utc::now().timestamp(),
        };

        // Get all nodes
        let nodes = self.graph.query_band(&RetrievalBand::default())?;
        result.nodes_processed = nodes.len();

        // Phase 1: Decay stale nodes
        for node in &nodes {
            let decay_amount = self.compute_decay(node);
            if decay_amount > 0.0 {
                let new_score = node.confidence.decay(decay_amount);
                if new_score.get() < node.confidence.get() {
                    result.nodes_decayed += 1;
                    let _ = self.trust.update_node_trust_layer(&node.id, new_score);
                }
            }
        }

        // Phase 2: Reinforce nodes with supporting evidence
        for node in &nodes {
            let effective = self.trust.effective_confidence(&node.id)?;
            if effective.get() > node.confidence.get() + 0.05 {
                let delta = (effective.get() - node.confidence.get()) * 0.3;
                let new_score = node.confidence.reinforce(delta);
                let _ = self.trust.update_node_trust_layer(&node.id, new_score);
            }
        }

        // Phase 3: Re-read nodes to check layer transitions
        let upgraded = self.count_layer_transitions(&nodes, true)?;
        let downgraded = self.count_layer_transitions(&nodes, false)?;
        result.nodes_upgraded = upgraded;
        result.nodes_downgraded = downgraded;

        // Phase 4: Prune weak edges
        result.edges_pruned = self.prune_weak_edges()?;

        // Phase 5: Increment anneal counts
        for node in &nodes {
            let _ = self.graph.increment_anneal_count(&node.id);
        }

        let elapsed = start.elapsed();
        info!(
            pass = ?result,
            elapsed_ms = elapsed.as_millis(),
            "Annealing pass complete"
        );

        Ok(result)
    }

    /// Run multiple annealing passes up to the configured maximum.
    pub fn run_full_anneal(&self) -> Result<Vec<AnnealResult>, GuardDbError> {
        let mut results = Vec::new();

        for i in 1..=self.config.max_anneal_passes {
            let result = self.run_pass()?;
            let pass_num = result.pass_number.max(i);
            let mut result = result;
            result.pass_number = pass_num;

            info!(pass = i, "Annealing pass complete");
            results.push(result);

            // Early termination: if no changes in a pass, we've converged
            if results.len() > 1 {
                let last = results.last().unwrap();
                let prev = results[results.len() - 2].clone();
                if last.nodes_upgraded == 0
                    && last.nodes_downgraded == 0
                    && last.nodes_decayed == 0
                    && last.edges_pruned == 0
                {
                    info!("Annealing converged after {} passes", i);
                    break;
                }
            }
        }

        Ok(results)
    }

    /// Selective fallback: when agent doubt is detected, request human review.
    /// Returns nodes that need review.
    pub fn doubt_fallback(&self, doubt_threshold: f64) -> Result<Vec<MemoryNode>, GuardDbError> {
        let conn = self.db.conn();

        let mut stmt = conn.prepare(
            "SELECT id, kind, content, trust_layer, confidence, created_at, last_touched, anneal_count, metadata
             FROM memory_nodes
             WHERE confidence < ?1 AND trust_layer >= 2
             ORDER BY confidence ASC
             LIMIT 20"
        )?;

        let nodes: Vec<MemoryNode> = stmt.query_map(params![doubt_threshold], |row| {
            Ok(MemoryNode {
                id: row.get(0)?,
                kind: self.parse_kind(row.get(1)?),
                content: row.get(2)?,
                trust_layer: row.get(3)?,
                confidence: TrustScore::new(row.get(4)?),
                created_at: row.get(5)?,
                last_touched: row.get(6)?,
                anneal_count: row.get(7)?,
                metadata: row.get(8)?,
            })
        })?.collect::<Result<_, _>>()?;

        if !nodes.is_empty() {
            warn!(
                count = nodes.len(),
                threshold = doubt_threshold,
                "Doubt fallback triggered for {} nodes",
                nodes.len()
            );
        }

        Ok(nodes)
    }

    /// Record an action outcome and update related node confidence.
    pub fn record_outcome(
        &self,
        outcome: &ActionOutcome,
    ) -> Result<(), GuardDbError> {
        let conn = self.db.conn();

        conn.execute(
            "INSERT INTO action_outcomes (id, action_id, success, exit_code, output_hash, residual, skill_residue, confidence_delta, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, unixepoch())",
            params![
                outcome.id,
                outcome.action_id,
                outcome.success as i32,
                outcome.exit_code,
                &outcome.output_hash,
                &outcome.residual,
                &outcome.skill_residue,
                outcome.confidence_delta,
            ],
        )?;

        // Update the action status
        conn.execute(
            "UPDATE action_requests SET status = 'executed', resolved_at = unixepoch() WHERE id = ?1",
            params![outcome.action_id],
        )?;

        // Reinforce or decay the source node based on outcome
        let source_id: Option<String> = conn.query_row(
            "SELECT source_event_id FROM action_requests WHERE id = ?1",
            params![outcome.action_id],
            |r| r.get(0),
        ).ok().flatten();

        if let Some(ref node_id) = source_id {
            if outcome.success {
                let _ = self.trust.reinforce(node_id, outcome.confidence_delta.abs());
            } else {
                let _ = self.trust.decay(node_id, outcome.confidence_delta.abs());
            }
        }

        debug!(outcome_id = outcome.id, success = outcome.success, "Action outcome recorded");
        Ok(())
    }

    /// Create an action request (gated by trust layer).
    pub fn request_action(
        &self,
        source_event_id: Option<String>,
        action_type: impl Into<String>,
        payload: impl Into<String>,
        trust_layer: u32,
        confidence: TrustScore,
    ) -> Result<ActionRequest, GuardDbError> {
        let id = uuid::Uuid::new_v4().to_string();
        let action_type = action_type.into();
        let payload = payload.into();

        let can_auto = self.trust.can_auto_execute(trust_layer)?;
        let needs_review = self.trust.requires_review(trust_layer)?;

        let status = if can_auto && !needs_review {
            ActionStatus::Approved
        } else if needs_review {
            ActionStatus::Pending
        } else {
            ActionStatus::Denied
        };

        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO action_requests (id, source_event_id, action_type, payload, trust_layer, confidence, status, requested_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, unixepoch())",
            params![
                id,
                &source_event_id,
                action_type,
                payload,
                trust_layer,
                confidence.get(),
                format!("{}", status),
            ],
        )?;

        let gate_result = match &status {
            ActionStatus::Approved => Some("auto-approved by trust layer".to_string()),
            ActionStatus::Pending => Some("pending human review".to_string()),
            ActionStatus::Denied => Some("denied by trust layer".to_string()),
            _ => None,
        };

        conn.execute(
            "UPDATE action_requests SET gate_result = ?1 WHERE id = ?2",
            params![gate_result, id],
        )?;

        Ok(ActionRequest {
            id,
            source_event_id,
            action_type,
            payload,
            trust_layer,
            confidence,
            status,
            gate_result,
            requested_at: chrono::Utc::now().timestamp(),
            resolved_at: None,
        })
    }

    // -- Internal helpers --

    fn compute_decay(&self, node: &MemoryNode) -> f64 {
        let now = chrono::Utc::now().timestamp();
        let age_seconds = now - node.last_touched;

        if age_seconds < self.config.anneal_interval_seconds as i64 {
            return 0.0;
        }

        let intervals = (age_seconds as f64) / (self.config.anneal_interval_seconds as f64);
        let base_decay = self.config.decay_rate * intervals;

        // Annealed nodes decay slower
        let anneal_factor = 1.0 / (1.0 + (node.anneal_count as f64) * 0.1);
        base_decay * anneal_factor
    }

    fn count_layer_transitions(&self, old_nodes: &[MemoryNode], upward: bool) -> Result<u32, GuardDbError> {
        let mut count = 0u32;

        for old in old_nodes {
            if let Some(new) = self.graph.get_node(&old.id)? {
                if upward && new.trust_layer > old.trust_layer {
                    count += 1;
                } else if !upward && new.trust_layer < old.trust_layer {
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    fn prune_weak_edges(&self) -> Result<u32, GuardDbError> {
        let conn = self.db.conn();
        let threshold = 0.1;

        let edges_to_prune: Vec<String> = conn.prepare(
            "SELECT id FROM memory_edges WHERE weight < ?1"
        )?.query_and_then(params![threshold], |row| {
            row.get::<_, String>(0)
        })?.collect();

        for edge_id in &edges_to_prune {
            conn.execute(
                "DELETE FROM memory_edges WHERE id = ?1",
                params![edge_id],
            )?;
        }

        Ok(edges_to_prune.len() as u32)
    }

    fn parse_kind(&self, kind_str: String) -> NodeKind {
        match kind_str.as_str() {
            "fact" => NodeKind::Fact,
            "pattern" => NodeKind::Pattern,
            "rule" => NodeKind::Rule,
            "reflection" => NodeKind::Reflection,
            "outcome" => NodeKind::Outcome,
            "residue" => NodeKind::Residue,
            _ => NodeKind::Fact,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_single_anneal_pass() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let mg = MemoryGraph::new(&db);

        mg.add_node(NodeKind::Fact, "test fact", TrustScore::new(0.7)).unwrap();
        mg.add_node(NodeKind::Rule, "test rule", TrustScore::new(0.5)).unwrap();

        let pipeline = AnnealingPipeline::new(&db).unwrap();
        let result = pipeline.run_pass().unwrap();

        assert!(result.nodes_processed >= 2);
    }

    #[test]
    fn test_action_request_auto_approve() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();

        let pipeline = AnnealingPipeline::new(&db).unwrap();
        let request = pipeline.request_action(
            None,
            "echo",
            "hello",
            2,
            TrustScore::new(0.7),
        ).unwrap();

        assert_eq!(request.status, ActionStatus::Approved);
    }

    #[test]
    fn test_action_request_pending_review() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();

        let pipeline = AnnealingPipeline::new(&db).unwrap();
        let request = pipeline.request_action(
            None,
            "rm",
            "-rf /tmp/test",
            0,
            TrustScore::new(0.1),
        ).unwrap();

        assert_eq!(request.status, ActionStatus::Pending);
    }

    #[test]
    fn test_outcome_reinforcement() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let mg = MemoryGraph::new(&db);
        let pipeline = AnnealingPipeline::new(&db).unwrap();

        let node_id = mg.add_node(NodeKind::Fact, "reinforce me", TrustScore::new(0.5)).unwrap();

        let request = pipeline.request_action(
            Some(node_id.clone()),
            "test_action",
            "payload",
            2,
            TrustScore::new(0.7),
        ).unwrap();

        let outcome = ActionOutcome {
            id: uuid::Uuid::new_v4().to_string(),
            action_id: request.id,
            success: true,
            exit_code: Some(0),
            output_hash: None,
            residual: None,
            skill_residue: Some("learned pattern".to_string()),
            confidence_delta: 0.1,
            created_at: chrono::Utc::now().timestamp(),
        };

        pipeline.record_outcome(&outcome).unwrap();

        let node = mg.get_node(&node_id).unwrap().unwrap();
        assert!(node.confidence.get() > 0.5);
    }
}
