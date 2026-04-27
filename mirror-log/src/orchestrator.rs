use crate::catalogue::{PromotionResult, find_promotion_candidates, promote_events};
use rusqlite::Connection;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("Catalogue error: {0}")]
    Catalogue(#[from] crate::catalogue::CatalogueError),
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Orchestration failed: {0}")]
    Runtime(String),
}

/// The PromotionOrchestrator manages the lifecycle of event promotion.
/// It identifies events that have matured through enough iterations and
/// triggers their consolidation into higher-level summary events.
pub struct PromotionOrchestrator<'a> {
    conn: &'a Connection,
}

impl<'a> PromotionOrchestrator<'a> {
    /// Creates a new orchestrator instance for the given database connection.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Executes a single promotion cycle.
    ///
    /// This method searches for candidate events that meet the iteration threshold,
    /// aggregates them, and creates a new summary event in the catalogue.
    pub fn run_promotion_cycle(
        &self,
        min_iterations: i32,
        summary_template: &str,
    ) -> std::result::Result<Option<PromotionResult>, OrchestratorError> {
        // 1. Find candidates that have reached the required maturity level
        let candidates = find_promotion_candidates(self.conn, min_iterations)?;

        if candidates.is_empty() {
            return Ok(None);
        }

        // 2. Prepare the summary content.
        // In a production implementation, this would involve calling an LLM/Inference engine.
        // For now, we use a template-based placeholder summarizing the count of events.
        let summary_content = summary_template.replace("{{count}}", &candidates.len().to_string());

        // 3. Define metadata for the new summary event.
        let summary_meta = serde_json::json!({
            "promoted_at": chrono::Utc::now().timestamp(),
            "source_count": candidates.len(),
            "orchestrator_version": "0.1.0-alpha"
        });

        // 4. Perform the promotion transactionally via the catalogue module
        let result = promote_events(self.conn, &candidates, &summary_content, Some(summary_meta))?;

        Ok(Some(result))
    }
}
