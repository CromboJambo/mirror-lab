use rusqlite::{Connection, params};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum CatalogueError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Event not found: {0}")]
    NotFound(String),
}

/// Represents the result of a successful event promotion.
#[derive(Debug, Clone)]
pub struct PromotionResult {
    pub summary_event_id: String,
    pub promoted_source_ids: Vec<String>,
}

/// Promotes a set of events into a single summary event.
/// The summary event links back to all source events via 'summary' relationship in `event_links`.
/// It also marks the source events as complete in their `iteration_status`.
pub fn promote_events(
    conn: &Connection,
    source_event_ids: &[String],
    summary_content: &str,
    summary_meta: Option<Value>,
) -> std::result::Result<PromotionResult, CatalogueError> {
    if source_event_ids.is_empty() {
        return Err(CatalogueError::NotFound(
            "No source events provided".to_string(),
        ));
    }

    let tx = conn.unchecked_transaction()?;

    let summary_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().timestamp();
    let meta_json = summary_meta.map(|m| m.to_string());

    // 1. Create the summary event
    // Note: content_hash is left empty for this placeholder implementation.
    tx.execute(
        "INSERT INTO events (id, timestamp, source, content, meta, ingested_at, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            summary_id,
            timestamp,
            "catalogue-promotion",
            summary_content,
            meta_json,
            timestamp,
            ""
        ],
    )?;

    let mut promoted_ids = Vec::new();

    for event_id in source_event_ids {
        // 2. Create the link from summary to source via `event_links`
        tx.execute(
            "INSERT INTO event_links (id, from_event_id, to_event_id, relation, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                summary_id,
                event_id,
                "summary",
                timestamp
            ],
        )?;

        // 3. Mark source event as complete in `iteration_status` if it exists
        tx.execute(
            "UPDATE iteration_status
             SET is_complete = 1, completion_reason = 'promotion', completed_at = ?1
             WHERE event_id = ?2",
            params![timestamp, event_id],
        )?;

        promoted_ids.push(event_id.clone());
    }

    tx.commit()?;

    Ok(PromotionResult {
        summary_event_id: summary_id,
        promoted_source_ids: promoted_ids,
    })
}

/// Finds events that are candidates for promotion based on their iteration count.
pub fn find_promotion_candidates(
    conn: &Connection,
    min_iterations: i32,
) -> std::result::Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT event_id FROM iteration_passes
         WHERE iteration_number >= ?1
         GROUP BY event_id
         HAVING COUNT(*) >= ?1",
    )?;

    let event_ids = stmt.query_map([min_iterations], |row| row.get(0))?;

    let mut results = Vec::new();
    for id in event_ids {
        results.push(id?);
    }

    Ok(results)
}
