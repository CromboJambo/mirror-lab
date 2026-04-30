use crate::guard_db::{GuardDb, GuardDbError};
use crate::memory::MemoryGraph;
use crate::types::*;

/// Retrieval bands provide layer-based querying of the memory graph.
///
/// Bands define trust/confidence ranges for retrieval, allowing flexible
/// and expensive queries when needed, or fast filtered lookups.
pub struct RetrievalEngine<'a> {
    db: &'a GuardDb,
    graph: MemoryGraph<'a>,
}

impl<'a> RetrievalEngine<'a> {
    pub fn new(db: &'a GuardDb) -> Self {
        Self {
            db,
            graph: MemoryGraph::new(db),
        }
    }

    /// Retrieve nodes within a trust band.
    pub fn retrieve(&self, band: &RetrievalBand) -> Result<Vec<MemoryNode>, GuardDbError> {
        self.graph.query_band(band)
    }

    /// Retrieve only annealed (high-trust) nodes.
    pub fn retrieve_annealed(&self, max: usize) -> Result<Vec<MemoryNode>, GuardDbError> {
        let mut band = RetrievalBand::annealed_only();
        band.max_results = max;
        self.retrieve(&band)
    }

    /// Retrieve working knowledge (auto-execute eligible).
    pub fn retrieve_working(&self, max: usize) -> Result<Vec<MemoryNode>, GuardDbError> {
        let mut band = RetrievalBand::working_and_above();
        band.max_results = max;
        self.retrieve(&band)
    }

    /// Retrieve all nodes regardless of trust (expensive, use sparingly).
    pub fn retrieve_all(&self, max: usize) -> Result<Vec<MemoryNode>, GuardDbError> {
        let band = RetrievalBand {
            max_results: max,
            ..RetrievalBand::default()
        };
        self.retrieve(&band)
    }

    /// Retrieve nodes of a specific kind within a trust band.
    pub fn retrieve_by_kind(
        &self,
        kind: NodeKind,
        band: &RetrievalBand,
    ) -> Result<Vec<MemoryNode>, GuardDbError> {
        let mut band = band.clone();
        band.kinds = Some(vec![kind]);
        self.retrieve(&band)
    }

    /// Retrieve nodes with their incoming evidence edges.
    pub fn retrieve_with_evidence(
        &self,
        node_ids: &[String],
    ) -> Result<Vec<(MemoryNode, Vec<MemoryEdge>)>, GuardDbError> {
        let mut results = Vec::new();

        for id in node_ids {
            if let Some(node) = self.graph.get_node(id)? {
                let evidence = self.graph.incoming_edges(id)?;
                results.push((node, evidence));
            }
        }

        Ok(results)
    }

    /// Content-based search with trust filtering.
    pub fn search(
        &self,
        query: &str,
        min_trust: u32,
        max_results: usize,
    ) -> Result<Vec<MemoryNode>, GuardDbError> {
        let all_results = self.graph.search_nodes(query, max_results * 2)?;
        let filtered: Vec<_> = all_results
            .into_iter()
            .filter(|n| n.trust_layer >= min_trust)
            .take(max_results)
            .collect();
        Ok(filtered)
    }

    /// Retrieve nodes that haven't been touched recently (stale detection).
    pub fn retrieve_stale(&self, max_age_seconds: i64, limit: usize) -> Result<Vec<MemoryNode>, GuardDbError> {
        let band = RetrievalBand {
            max_results: limit,
            ..RetrievalBand::default()
        };

        let nodes = self.graph.query_band(&band)?;
        let now = chrono::Utc::now().timestamp();

        Ok(nodes
            .into_iter()
            .filter(|n| (now - n.last_touched) > max_age_seconds)
            .collect())
    }

    /// Get a summary of the memory graph distribution across trust layers.
    pub fn trust_distribution(&self) -> Result<Vec<(u32, String, u64)>, GuardDbError> {
        let conn = self.db_conn();
        let mut stmt = conn.prepare(
            "SELECT tl.id, tl.name, COUNT(n.id) as cnt
             FROM trust_layers tl
             LEFT JOIN memory_nodes n ON tl.id = n.trust_layer
             GROUP BY tl.id
             ORDER BY tl.id"
        )?;

        let dist: Vec<(u32, String, u64)> = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?.collect::<Result<_, _>>()?;

        Ok(dist)
    }

    fn db_conn(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.db.conn()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard_db::GuardDb;
    use tempfile::tempdir;

    #[test]
    fn test_retrieve_annealed() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let engine = RetrievalEngine::new(&db);
        let mg = MemoryGraph::new(&db);

        mg.add_node(NodeKind::Fact, "low trust", TrustScore::new(0.2)).unwrap();
        mg.add_node(NodeKind::Fact, "high trust", TrustScore::new(0.9)).unwrap();

        let annealed = engine.retrieve_annealed(10).unwrap();
        assert!(annealed.iter().all(|n| n.confidence.get() >= 0.8));
    }

    #[test]
    fn test_search_with_trust_filter() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let engine = RetrievalEngine::new(&db);
        let mg = MemoryGraph::new(&db);

        mg.add_node(NodeKind::Fact, "rust search term", TrustScore::new(0.9)).unwrap();
        mg.add_node(NodeKind::Fact, "rust low trust", TrustScore::new(0.2)).unwrap();

        let results = engine.search("rust", 2, 10).unwrap();
        assert!(results.iter().all(|n| n.trust_layer >= 2));
    }
}
