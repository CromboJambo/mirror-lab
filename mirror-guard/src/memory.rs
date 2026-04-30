use rusqlite::params;
use uuid::Uuid;

use crate::guard_db::{GuardDb, GuardDbError};
use crate::types::*;

/// Manages the memory graph: nodes, edges, and graph queries.
pub struct MemoryGraph<'a> {
    db: &'a GuardDb,
}

impl<'a> MemoryGraph<'a> {
    pub fn new(db: &'a GuardDb) -> Self {
        Self { db }
    }

    // -- Node operations --

    /// Add a new memory node.
    pub fn add_node(
        &self,
        kind: NodeKind,
        content: impl Into<String>,
        confidence: TrustScore,
    ) -> Result<String, GuardDbError> {
        let id = Uuid::new_v4().to_string();
        let content = content.into();
        let conn = self.db.conn();

        conn.execute(
            "INSERT INTO memory_nodes (id, kind, content, confidence, created_at, last_touched)
             VALUES (?1, ?2, ?3, ?4, unixepoch(), unixepoch())",
            params![id, format!("{}", kind), content, confidence.get()],
        )?;

        Ok(id)
    }

    /// Get a memory node by ID.
    pub fn get_node(&self, id: &str) -> Result<Option<MemoryNode>, GuardDbError> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, kind, content, trust_layer, confidence, created_at, last_touched, anneal_count, metadata FROM memory_nodes WHERE id = ?1"
        )?;

        let node = stmt
            .query_row(params![id], |row| {
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
            })
            .ok();

        Ok(node)
    }

    /// Update node content and metadata.
    pub fn update_node(
        &self,
        id: &str,
        content: Option<impl Into<String>>,
        metadata: Option<impl Into<String>>,
    ) -> Result<(), GuardDbError> {
        let conn = self.db.conn();

        let mut updates = Vec::new();

        if let Some(c) = content {
            updates.push(format!("content = '{}'", c.into().replace('\'', "''")));
        }
        if let Some(m) = metadata {
            updates.push(format!("metadata = '{}'", m.into().replace('\'', "''")));
        }

        if updates.is_empty() {
            return Ok(());
        }

        updates.push("last_touched = unixepoch()".to_string());
        let set_clause = updates.join(", ");

        conn.execute(
            &format!("UPDATE memory_nodes SET {} WHERE id = ?1", set_clause),
            params![id],
        )?;

        Ok(())
    }

    // -- Edge operations --

    /// Add a directed edge between two nodes.
    pub fn add_edge(
        &self,
        from_id: &str,
        to_id: &str,
        relation: EdgeRelation,
        weight: f64,
    ) -> Result<String, GuardDbError> {
        let id = Uuid::new_v4().to_string();
        let conn = self.db.conn();

        conn.execute(
            "INSERT INTO memory_edges (id, from_id, to_id, relation, weight, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, unixepoch())",
            params![id, from_id, to_id, format!("{}", relation), weight],
        )?;

        Ok(id)
    }

    /// Remove an edge by ID.
    pub fn remove_edge(&self, edge_id: &str) -> Result<bool, GuardDbError> {
        let conn = self.db.conn();
        let rows = conn.execute("DELETE FROM memory_edges WHERE id = ?1", params![edge_id])?;
        Ok(rows > 0)
    }

    /// Get all edges from a node.
    pub fn outgoing_edges(&self, node_id: &str) -> Result<Vec<MemoryEdge>, GuardDbError> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, from_id, to_id, relation, weight, created_at FROM memory_edges WHERE from_id = ?1"
        )?;

        let edges: Vec<MemoryEdge> = stmt
            .query_map(params![node_id], |row| {
                Ok(MemoryEdge {
                    id: row.get(0)?,
                    from_id: row.get(1)?,
                    to_id: row.get(2)?,
                    relation: self.parse_relation(row.get(3)?),
                    weight: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<Result<_, _>>()?;

        Ok(edges)
    }

    /// Get all edges to a node.
    pub fn incoming_edges(&self, node_id: &str) -> Result<Vec<MemoryEdge>, GuardDbError> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, from_id, to_id, relation, weight, created_at FROM memory_edges WHERE to_id = ?1"
        )?;

        let edges: Vec<MemoryEdge> = stmt
            .query_map(params![node_id], |row| {
                Ok(MemoryEdge {
                    id: row.get(0)?,
                    from_id: row.get(1)?,
                    to_id: row.get(2)?,
                    relation: self.parse_relation(row.get(3)?),
                    weight: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<Result<_, _>>()?;

        Ok(edges)
    }

    // -- Query operations --

    /// Query nodes matching a retrieval band.
    pub fn query_band(&self, band: &RetrievalBand) -> Result<Vec<MemoryNode>, GuardDbError> {
        let conn = self.db.conn();

        let mut query = String::from(
            "SELECT n.id, n.kind, n.content, n.trust_layer, n.confidence, n.created_at, n.last_touched, n.anneal_count, n.metadata
             FROM memory_nodes n
             WHERE n.trust_layer >= ? AND n.trust_layer <= ? AND n.confidence >= ?"
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(band.min_trust_layer),
            Box::new(band.max_trust_layer),
            Box::new(band.min_confidence),
        ];

        if let Some(ref kinds) = band.kinds {
            query.push_str(&format!(
                " AND n.kind IN ({})",
                (0..kinds.len())
                    .map(|_| "?".to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            for kind in kinds {
                params.push(Box::new(format!("{}", kind)));
            }
        }

        query.push_str(&format!(
            " ORDER BY n.confidence DESC LIMIT {}",
            band.max_results
        ));

        let mut stmt = conn.prepare(&query)?;
        let nodes: Vec<MemoryNode> = stmt
            .query_map(
                rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                |row| {
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
                },
            )?
            .collect::<Result<_, _>>()?;

        Ok(nodes)
    }

    /// Search nodes by content substring.
    pub fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<MemoryNode>, GuardDbError> {
        let conn = self.db.conn();
        let pattern = format!("%{}%", query);

        let mut stmt = conn.prepare(
            "SELECT id, kind, content, trust_layer, confidence, created_at, last_touched, anneal_count, metadata
             FROM memory_nodes
             WHERE content LIKE ?1
             ORDER BY confidence DESC
             LIMIT ?2"
        )?;

        let nodes: Vec<MemoryNode> = stmt
            .query_map(params![pattern, limit], |row| {
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
            })?
            .collect::<Result<_, _>>()?;

        Ok(nodes)
    }

    /// Increment anneal count for a node (called during annealing passes).
    pub fn increment_anneal_count(&self, node_id: &str) -> Result<u32, GuardDbError> {
        let conn = self.db.conn();
        let new_count: u32 = conn
            .query_row(
                "SELECT anneal_count + 1 FROM memory_nodes WHERE id = ?1",
                params![node_id],
                |r| r.get(0),
            )
            .map_err(|_| GuardDbError::SchemaError("Node not found".into()))?;

        conn.execute(
            "UPDATE memory_nodes SET anneal_count = ?1, last_touched = unixepoch() WHERE id = ?2",
            params![new_count, node_id],
        )?;

        Ok(new_count)
    }

    /// Count total nodes.
    pub fn node_count(&self) -> Result<u64, GuardDbError> {
        let conn = self.db.conn();
        let count: u64 = conn.query_row("SELECT count(*) FROM memory_nodes", [], |r| r.get(0))?;
        Ok(count)
    }

    // -- Helpers --

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

    fn parse_relation(&self, rel_str: String) -> EdgeRelation {
        match rel_str.as_str() {
            "supports" => EdgeRelation::Supports,
            "contradicts" => EdgeRelation::Contradicts,
            "derived_from" => EdgeRelation::DerivedFrom,
            "anneals" => EdgeRelation::Anneals,
            "depends_on" => EdgeRelation::DependsOn,
            "evidence_for" => EdgeRelation::EvidenceFor,
            _ => EdgeRelation::Supports,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_add_and_get_node() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let mg = MemoryGraph::new(&db);

        let id = mg
            .add_node(NodeKind::Fact, "hello world", TrustScore::new(0.7))
            .unwrap();
        let node = mg.get_node(&id).unwrap().unwrap();

        assert_eq!(node.kind, NodeKind::Fact);
        assert_eq!(node.content, "hello world");
        assert!((node.confidence.get() - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_add_edge_and_query() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let mg = MemoryGraph::new(&db);

        let a = mg
            .add_node(NodeKind::Fact, "A", TrustScore::new(0.8))
            .unwrap();
        let b = mg
            .add_node(NodeKind::Fact, "B", TrustScore::new(0.6))
            .unwrap();

        mg.add_edge(&a, &b, EdgeRelation::Supports, 0.9).unwrap();

        let outgoing = mg.outgoing_edges(&a).unwrap();
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].relation, EdgeRelation::Supports);

        let incoming = mg.incoming_edges(&b).unwrap();
        assert_eq!(incoming.len(), 1);
    }

    #[test]
    fn test_query_band_filtering() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let mg = MemoryGraph::new(&db);

        mg.add_node(NodeKind::Fact, "low confidence", TrustScore::new(0.2))
            .unwrap();
        mg.add_node(NodeKind::Fact, "high confidence", TrustScore::new(0.9))
            .unwrap();
        mg.add_node(NodeKind::Rule, "a rule", TrustScore::new(0.7))
            .unwrap();

        let band = RetrievalBand::annealed_only();
        let results = mg.query_band(&band).unwrap();
        assert!(results.iter().all(|n| n.confidence.get() >= 0.8));
    }

    #[test]
    fn test_search_nodes() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let mg = MemoryGraph::new(&db);

        mg.add_node(NodeKind::Fact, "rust is great", TrustScore::new(0.8))
            .unwrap();
        mg.add_node(NodeKind::Fact, "python is ok", TrustScore::new(0.6))
            .unwrap();

        let results = mg.search_nodes("rust", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("rust"));
    }

    #[test]
    fn test_increment_anneal_count() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let mg = MemoryGraph::new(&db);

        let id = mg
            .add_node(NodeKind::Fact, "test", TrustScore::new(0.5))
            .unwrap();

        let count1 = mg.increment_anneal_count(&id).unwrap();
        assert_eq!(count1, 1);

        let count2 = mg.increment_anneal_count(&id).unwrap();
        assert_eq!(count2, 2);
    }
}
