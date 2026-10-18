use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MirrorLogMcpServer {
    db_path: PathBuf,
}

impl MirrorLogMcpServer {
    fn new() -> Self {
        let db_path = env::var("MIRROR_LOG_DB_PATH")
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .map(|h| h.join(".hermes/mirror-log.db").to_string_lossy().to_string())
                    .unwrap_or("/tmp/mirror-log.db".to_string())
            });

        Self {
            db_path: PathBuf::from(db_path),
        }
    }

    fn get_connection(&self) -> Result<Connection, rusqlite::Error> {
        Connection::open(&self.db_path)
    }
}

// Request/Response types

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LogObservationRequest {
    pub source: String,
    pub content: String,
    #[serde(default)]
    pub meta: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetObservationRequest {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListObservationsRequest {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i32,
}

fn default_limit() -> i32 { 50 }

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UpdateObservationRequest {
    pub id: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub meta: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeleteObservationRequest {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CalculateAttentionScoreRequest {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TrackAccessRequest {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PinObservationRequest {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UnpinObservationRequest {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LogEventRequest {
    pub event_type: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SyncFleetStateRequest {
    pub node_id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EmptyRequest {}

// MCP Server Implementation

#[tool_router(server_handler)]
impl MirrorLogMcpServer {
    #[tool(description = "Record new observation with metadata")]
    fn log_observation(&self, params: Parameters<LogObservationRequest>) -> String {
        let req = &params.0;
        match self.get_connection() {
            Ok(conn) => {
                let meta_str = req.meta.as_deref();
                match mirror_log::append(&conn, &req.source, &req.content, meta_str) {
                    Ok(id) => serde_json::to_string_pretty(&serde_json::json!({
                        "id": id,
                        "source": req.source,
                        "status": "logged"
                    }))
                    .unwrap_or_default(),
                    Err(e) => format!("Error: {}", e),
                }
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "Retrieve specific observation by ID")]
    fn get_observation(&self, params: Parameters<GetObservationRequest>) -> String {
        let req = &params.0;
        match self.get_connection() {
            Ok(conn) => {
                conn.query_row(
                    "SELECT id, timestamp, source, content, meta FROM events WHERE id = ?1",
                    params![req.id],
                    |row| {
                        Ok(serde_json::json!({
                            "id": row.get::<_, String>(0)?,
                            "timestamp": row.get::<_, i64>(1)?,
                            "source": row.get::<_, String>(2)?,
                            "content": row.get::<_, String>(3)?,
                            "meta": row.get::<_, Option<String>>(4)?
                        }))
                    },
                )
                .map(|v| v.to_string())
                .unwrap_or_else(|e| format!("Error: {}", e))
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "Query recent observations")]
    fn list_observations(&self, params: Parameters<ListObservationsRequest>) -> String {
        let req = &params.0;
        match self.get_connection() {
            Ok(conn) => {
                if let Some(source) = &req.source {
                    let mut stmt = match conn.prepare(
                        "SELECT id, timestamp, source, content, meta FROM events WHERE source = ?1 ORDER BY timestamp DESC LIMIT ?2",
                    ) {
                        Ok(s) => s,
                        Err(e) => return format!("Prepare error: {}", e),
                    };

                    let rows = match stmt.query_map(params![source, req.limit], |row| {
                        Ok(serde_json::json!({
                            "id": row.get::<_, String>(0)?,
                            "timestamp": row.get::<_, i64>(1)?,
                            "source": row.get::<_, String>(2)?,
                            "content": row.get::<_, String>(3)?,
                            "meta": row.get::<_, Option<String>>(4)?
                        }))
                    }) {
                        Ok(r) => r,
                        Err(e) => return format!("Query error: {}", e),
                    };

                    let mut results = vec![];
                    for row in rows {
                        match row {
                            Ok(json) => results.push(json),
                            Err(e) => return format!("Row error: {}", e),
                        }
                    }
                    serde_json::to_string_pretty(&results).unwrap_or_default()
                } else {
                    let mut stmt = match conn.prepare(
                        "SELECT id, timestamp, source, content, meta FROM events ORDER BY timestamp DESC LIMIT ?1",
                    ) {
                        Ok(s) => s,
                        Err(e) => return format!("Prepare error: {}", e),
                    };

                    let rows = match stmt.query_map(params![req.limit], |row| {
                        Ok(serde_json::json!({
                            "id": row.get::<_, String>(0)?,
                            "timestamp": row.get::<_, i64>(1)?,
                            "source": row.get::<_, String>(2)?,
                            "content": row.get::<_, String>(3)?,
                            "meta": row.get::<_, Option<String>>(4)?
                        }))
                    }) {
                        Ok(r) => r,
                        Err(e) => return format!("Query error: {}", e),
                    };

                    let mut results = vec![];
                    for row in rows {
                        match row {
                            Ok(json) => results.push(json),
                            Err(e) => return format!("Row error: {}", e),
                        }
                    }
                    serde_json::to_string_pretty(&results).unwrap_or_default()
                }
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "Modify existing observation")]
    fn update_observation(&self, params: Parameters<UpdateObservationRequest>) -> String {
        let req = &params.0;
        match self.get_connection() {
            Ok(conn) => {
                if let Some(content) = &req.content {
                    conn.execute(
                        "UPDATE events SET content = ?1 WHERE id = ?2",
                        params![content, req.id],
                    )
                    .map(|_| format!("Updated observation {}", req.id))
                    .unwrap_or_else(|e| format!("Error: {}", e))
                } else if let Some(meta) = &req.meta {
                    conn.execute(
                        "UPDATE events SET meta = ?1 WHERE id = ?2",
                        params![meta, req.id],
                    )
                    .map(|_| format!("Updated observation {}", req.id))
                    .unwrap_or_else(|e| format!("Error: {}", e))
                } else {
                    "No fields to update".to_string()
                }
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "Remove observation")]
    fn delete_observation(&self, params: Parameters<DeleteObservationRequest>) -> String {
        let req = &params.0;
        match self.get_connection() {
            Ok(conn) => {
                conn.execute("DELETE FROM events WHERE id = ?1", params![req.id])
                    .map(|rows| format!("Deleted {} row(s)", rows))
                    .unwrap_or_else(|e| format!("Error: {}", e))
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "Compute attention score for observation")]
    fn calculate_attention_score(&self, params: Parameters<CalculateAttentionScoreRequest>) -> String {
        let req = &params.0;
        match self.get_connection() {
            Ok(conn) => {
                let layer = mirror_log::attention::AttentionLayer::default();
                match layer.calculate_attention_score(&conn, &req.id) {
                    Ok(score) => serde_json::to_string_pretty(&serde_json::json!({
                        "id": req.id,
                        "score": score
                    }))
                    .unwrap_or_default(),
                    Err(e) => format!("Error: {}", e),
                }
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "List observations above threshold")]
    fn get_high_attention(&self, _params: Parameters<EmptyRequest>) -> String {
        match self.get_connection() {
            Ok(conn) => {
                let layer = mirror_log::attention::AttentionLayer::default();
                match layer.get_active_items(&conn) {
                    Ok(items) => serde_json::to_string_pretty(&items).unwrap_or_default(),
                    Err(e) => format!("Error: {}", e),
                }
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "Fleet-wide attention statistics")]
    fn get_attention_stats(&self, _params: Parameters<EmptyRequest>) -> String {
        match self.get_connection() {
            Ok(conn) => {
                let layer = mirror_log::attention::AttentionLayer::default();
                match layer.get_stats(&conn) {
                    Ok(stats) => serde_json::to_string_pretty(&stats).unwrap_or_default(),
                    Err(e) => format!("Error: {}", e),
                }
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "Record access event (affects decay)")]
    fn track_access(&self, params: Parameters<TrackAccessRequest>) -> String {
        let req = &params.0;
        match self.get_connection() {
            Ok(conn) => {
                mirror_log::decay::track_access(&conn, &req.id)
                    .map(|_| format!("Tracked access for {}", req.id))
                    .unwrap_or_else(|e| format!("Error: {}", e))
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "Record event with type and payload")]
    fn log_event(&self, params: Parameters<LogEventRequest>) -> String {
        let req = &params.0;
        match self.get_connection() {
            Ok(conn) => {
                let payload_str = serde_json::to_string(&req.payload).unwrap_or_default();
                match mirror_log::append(&conn, "event", &payload_str, None) {
                    Ok(id) => serde_json::to_string_pretty(&serde_json::json!({
                        "id": id,
                        "type": req.event_type,
                        "status": "logged"
                    }))
                    .unwrap_or_default(),
                    Err(e) => format!("Error: {}", e),
                }
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "Prevent decay for critical items")]
    fn pin_observation(&self, params: Parameters<PinObservationRequest>) -> String {
        let req = &params.0;
        match self.get_connection() {
            Ok(conn) => {
                mirror_log::decay::pin_event(&conn, &req.id)
                    .map(|_| format!("Pinned observation {}", req.id))
                    .unwrap_or_else(|e| format!("Error: {}", e))
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "Restore normal decay behavior")]
    fn unpin_observation(&self, params: Parameters<UnpinObservationRequest>) -> String {
        let req = &params.0;
        match self.get_connection() {
            Ok(conn) => {
                mirror_log::decay::unpin_event(&conn, &req.id)
                    .map(|_| format!("Unpinned observation {}", req.id))
                    .unwrap_or_else(|e| format!("Error: {}", e))
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "Force global decay cycle")]
    fn decay_all(&self, _params: Parameters<EmptyRequest>) -> String {
        match self.get_connection() {
            Ok(conn) => {
                let config = mirror_log::decay::DecayConfig::default();
                match mirror_log::decay::move_to_shadow(&conn, &config) {
                    Ok(count) => format!("Decayed {} items to shadow", count),
                    Err(e) => format!("Error: {}", e),
                }
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "Aggregate attention across fleet nodes")]
    fn sync_fleet_state(&self, params: Parameters<SyncFleetStateRequest>) -> String {
        let req = &params.0;
        match self.get_connection() {
            Ok(conn) => {
                match mirror_log::append(&conn, "fleet_sync", &req.node_id, None) {
                    Ok(id) => serde_json::to_string_pretty(&serde_json::json!({
                        "id": id,
                        "node_id": req.node_id,
                        "status": "synced"
                    }))
                    .unwrap_or_default(),
                    Err(e) => format!("Error: {}", e),
                }
            }
            Err(e) => format!("Database error: {}", e),
        }
    }

    #[tool(description = "Cross-node high-attention items")]
    fn get_fleet_high_attention(&self, _params: Parameters<EmptyRequest>) -> String {
        self.get_high_attention(Parameters(EmptyRequest {}))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = MirrorLogMcpServer::new();

    let running = rmcp::serve_server(server, (tokio::io::stdin(), tokio::io::stdout())).await?;

    // Wait for the service loop to terminate (e.g. when stdin closes)
    running.waiting().await?;

    Ok(())
}