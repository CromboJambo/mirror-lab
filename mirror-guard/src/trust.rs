use rusqlite::params;
use tracing::{debug, info, warn};

use crate::guard_db::{GuardDb, GuardDbError};
use crate::types::*;

/// Manages trust layers and confidence scoring for memory nodes.
/// Trust layers define bands of confidence that determine auto-execute and review behavior.
pub struct TrustManager<'a> {
    db: &'a GuardDb,
}

impl<'a> TrustManager<'a> {
    pub fn new(db: &'a GuardDb) -> Self {
        Self { db }
    }

    /// List all configured trust layers.
    pub fn list_layers(&self) -> Result<Vec<TrustLayer>, GuardDbError> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, min_confidence, max_confidence, auto_execute, requires_review, description FROM trust_layers ORDER BY id"
        )?;

        let layers: Vec<TrustLayer> = stmt.query_map([], |row| {
            Ok(TrustLayer {
                id: row.get(0)?,
                name: row.get(1)?,
                min_confidence: row.get(2)?,
                max_confidence: row.get(3)?,
                auto_execute: row.get(4)?,
                requires_review: row.get(5)?,
                description: row.get(6)?,
            })
        })?.collect::<Result<_, _>>()?;

        Ok(layers)
    }

    /// Find the trust layer for a given confidence score.
    pub fn layer_for_score(&self, score: TrustScore) -> Result<Option<TrustLayer>, GuardDbError> {
        let layers = self.list_layers()?;
        for layer in &layers {
            if layer.contains_score(score) {
                return Ok(Some(layer.clone()));
            }
        }
        // If score exceeds all layers, return the highest layer
        layers.last().cloned().map(Some).ok_or(GuardDbError::SchemaError(
            "No trust layers configured".into(),
        ))
    }

    /// Check if a node at a given trust layer can auto-execute.
    pub fn can_auto_execute(&self, trust_layer_id: u32) -> Result<bool, GuardDbError> {
        let conn = self.db.conn();
        let auto: bool = conn.query_row(
            "SELECT auto_execute FROM trust_layers WHERE id = ?1",
            params![trust_layer_id],
            |r| r.get(0),
        ).map_err(|_| GuardDbError::SchemaError("Trust layer not found".into()))?;
        Ok(auto)
    }

    /// Check if a node at a given trust layer requires human review.
    pub fn requires_review(&self, trust_layer_id: u32) -> Result<bool, GuardDbError> {
        let conn = self.db.conn();
        let review: bool = conn.query_row(
            "SELECT requires_review FROM trust_layers WHERE id = ?1",
            params![trust_layer_id],
            |r| r.get(0),
        ).map_err(|_| GuardDbError::SchemaError("Trust layer not found".into()))?;
        Ok(review)
    }

    /// Update a node's trust layer based on its current confidence score.
    /// Returns the old and new layer IDs.
    pub fn update_node_trust_layer(
        &self,
        node_id: &str,
        new_confidence: TrustScore,
    ) -> Result<Option<(u32, u32)>, GuardDbError> {
        let conn = self.db.conn();

        let old_layer: u32 = conn.query_row(
            "SELECT trust_layer FROM memory_nodes WHERE id = ?1",
            params![node_id],
            |r| r.get(0),
        ).map_err(|_| GuardDbError::SchemaError("Node not found".into()))?;

        let new_layer = self.layer_for_score(new_confidence)?
            .map(|l| l.id)
            .unwrap_or(old_layer);

        if old_layer != new_layer {
            info!(
                node = node_id,
                old_layer = old_layer,
                new_layer = new_layer,
                confidence = new_confidence.get(),
                "Trust layer transition"
            );
        }

        conn.execute(
            "UPDATE memory_nodes SET confidence = ?1, trust_layer = ?2, last_touched = unixepoch() WHERE id = ?3",
            params![new_confidence.get(), new_layer, node_id],
        )?;

        if old_layer != new_layer {
            Ok(Some((old_layer, new_layer)))
        } else {
            Ok(None)
        }
    }

    /// Record a human review of a memory node.
    pub fn record_review(
        &self,
        record: &ReviewRecord,
    ) -> Result<(), GuardDbError> {
        let conn = self.db.conn();

        conn.execute(
            "INSERT INTO review_records (id, node_id, reviewer, action, old_confidence, new_confidence, old_trust_layer, new_trust_layer, notes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, unixepoch())",
            params![
                record.id,
                record.node_id,
                record.reviewer,
                format!("{}", record.action),
                record.old_confidence.map(|s| s.get()),
                record.new_confidence.map(|s| s.get()),
                record.old_trust_layer,
                record.new_trust_layer,
                &record.notes,
            ],
        )?;

        debug!(
            review_id = record.id,
            node = record.node_id,
            action = %record.action,
            "Review recorded"
        );

        Ok(())
    }

    /// Compute effective confidence for a node, factoring in supporting and contradicting evidence.
    pub fn effective_confidence(&self, node_id: &str) -> Result<TrustScore, GuardDbError> {
        let conn = self.db.conn();

        let base_confidence: f64 = conn.query_row(
            "SELECT confidence FROM memory_nodes WHERE id = ?1",
            params![node_id],
            |r| r.get(0),
        ).map_err(|_| GuardDbError::SchemaError("Node not found".into()))?;

        let support_sum: f64 = conn.query_row(
            "SELECT COALESCE(SUM(e.weight * mn.confidence), 0)
             FROM memory_edges e
             JOIN memory_nodes mn ON e.from_id = mn.id
             WHERE e.to_id = ?1 AND e.relation = 'supports'",
            params![node_id],
            |r| r.get(0),
        ).unwrap_or(0.0);

        let contradict_sum: f64 = conn.query_row(
            "SELECT COALESCE(SUM(e.weight * mn.confidence), 0)
             FROM memory_edges e
             JOIN memory_nodes mn ON e.from_id = mn.id
             WHERE e.to_id = ?1 AND e.relation = 'contradicts'",
            params![node_id],
            |r| r.get(0),
        ).unwrap_or(0.0);

        let total_evidence = support_sum + contradict_sum;
        let effective = if total_evidence > 0.0 {
            let evidence_ratio = support_sum / total_evidence;
            0.6 * base_confidence + 0.4 * evidence_ratio
        } else {
            base_confidence
        };

        Ok(TrustScore::new(effective))
    }

    /// Reinforce a node's confidence after a successful outcome.
    pub fn reinforce(&self, node_id: &str, delta: f64) -> Result<TrustScore, GuardDbError> {
        let conn = self.db.conn();

        let current: f64 = conn.query_row(
            "SELECT confidence FROM memory_nodes WHERE id = ?1",
            params![node_id],
            |r| r.get(0),
        ).map_err(|_| GuardDbError::SchemaError("Node not found".into()))?;

        let new_score = TrustScore::new(current + delta);
        debug!(node = node_id, old = current, new = new_score.get(), "Confidence reinforced");

        self.update_node_trust_layer(node_id, new_score)?;
        Ok(new_score)
    }

    /// Decay a node's confidence based on time and usage.
    pub fn decay(&self, node_id: &str, rate: f64) -> Result<TrustScore, GuardDbError> {
        let conn = self.db.conn();

        let current: f64 = conn.query_row(
            "SELECT confidence FROM memory_nodes WHERE id = ?1",
            params![node_id],
            |r| r.get(0),
        ).map_err(|_| GuardDbError::SchemaError("Node not found".into()))?;

        let config = self.db.load_anneal_config()?;
        let new_score = TrustScore::new((current - rate).max(config.confidence_floor));
        warn!(node = node_id, old = current, new = new_score.get(), "Confidence decayed");

        self.update_node_trust_layer(node_id, new_score)?;
        Ok(new_score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryGraph;
    use tempfile::tempdir;

    #[test]
    fn test_list_default_layers() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let tm = TrustManager::new(&db);

        let layers = tm.list_layers().unwrap();
        assert!(layers.len() >= 4);
        assert_eq!(layers[0].name(), "raw");
        assert!(!layers[0].auto_execute);
        assert!(layers[2].auto_execute);
    }

    #[test]
    fn test_layer_for_score() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let tm = TrustManager::new(&db);

        let raw = tm.layer_for_score(TrustScore::new(0.1)).unwrap().unwrap();
        assert_eq!(raw.name(), "raw");

        let working = tm.layer_for_score(TrustScore::new(0.6)).unwrap().unwrap();
        assert_eq!(working.name(), "working");

        let annealed = tm.layer_for_score(TrustScore::new(0.9)).unwrap().unwrap();
        assert_eq!(annealed.name(), "annealed");
    }

    #[test]
    fn test_update_trust_layer_transition() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let mg = MemoryGraph::new(&db);
        let tm = TrustManager::new(&db);

        let node_id = mg.add_node(NodeKind::Fact, "test fact", TrustScore::new(0.3)).unwrap();

        let result = tm.update_node_trust_layer(&node_id, TrustScore::new(0.75)).unwrap();
        assert!(result.is_some());
        let (old, new) = result.unwrap();
        assert_eq!(old, 1);
        assert_eq!(new, 2);
    }

    #[test]
    fn test_effective_confidence_with_evidence() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let mg = MemoryGraph::new(&db);
        let tm = TrustManager::new(&db);

        let target = mg.add_node(NodeKind::Fact, "target", TrustScore::new(0.5)).unwrap();
        let supporter = mg.add_node(NodeKind::Fact, "evidence for target", TrustScore::new(0.9)).unwrap();

        mg.add_edge(&supporter, &target, EdgeRelation::Supports, 0.8).unwrap();

        let effective = tm.effective_confidence(&target).unwrap();
        assert!(effective.get() > 0.5);
    }
}
