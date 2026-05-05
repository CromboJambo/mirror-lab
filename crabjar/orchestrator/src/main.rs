use axum::{
    Router,
    extract::Json,
    response::sse::{Event as SseEvent, Sse},
    routing::post,
};
use futures_util::stream;
use mirror_guard::{ActionStatus, ExecutionGate, GateContext};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::net::SocketAddr;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Represents the incoming request to run a command.
#[derive(Debug, Deserialize)]
struct RunRequest {
    tool: String,
    args: Vec<String>,
}

/// Represents the incoming request for a prompt.
#[derive(Debug, Deserialize)]
struct PromptRequest {
    message: String,
}

/// Represents an OpenAI-compatible Chat Completion request.
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

/// Represents a single message in the chat history.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ChatMessage {
    role: MessageRole,
    content: String,
}

/// Represents the incoming request for a chat interaction.
#[derive(Debug, Deserialize)]
struct ChatRequest {
    prompt: String,
    model: Option<String>,
}

/// Represents an OpenAI-compatible Chat Completion response content.
#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    r#type: String,
    function: FunctionCall,
}

#[derive(Debug, Deserialize)]
struct FunctionCall {
    name: String,
    arguments: String,
}

/// Represents a search_logs request.
#[derive(Debug, Deserialize)]
struct SearchLogsRequest {
    term: String,
    limit: Option<i64>,
}

/// Represents a recent_events request.
#[derive(Debug, Deserialize)]
struct RecentEventsRequest {
    limit: i64,
}

/// Represents a by_source request.
#[derive(Debug, Deserialize)]
struct BySourceRequest {
    source: String,
    limit: Option<i64>,
}

/// Represents the outgoing messages from the Orchestrator to the Client via SSE or JSON.
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
enum AcpResponse {
    /// Send real-time output/logs.
    Output { data: String },
    /// Ask the user for input.
    Input { message: String },
    /// Signal task finalization.
    Done { status: String },
    /// Report an error.
    Error { error: String },
}

// ---------------------------------------------------------------------------
// Gate concierge — provenance boundary enforcement
// ---------------------------------------------------------------------------
mod concierge;

// ---------------------------------------------------------------------------
// Local SQLite event store (replaces mirror-log dependency)
// ---------------------------------------------------------------------------

/// A lightweight SQLite-backed event store for the orchestrator.
/// Provides search, recent events, and by-source queries without
/// requiring the full mirror-log crate.
mod local_log {
    use rusqlite::{Connection, params};

    /// Initialize or open the event database.
    pub fn init_db(db_path: &str) -> Result<Connection, String> {
        let conn = Connection::open(db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                source TEXT NOT NULL,
                content TEXT NOT NULL,
                preview TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_events_source ON events(source);
            CREATE INDEX IF NOT EXISTS idx_events_content ON events(content);",
        )
        .map_err(|e| format!("Failed to initialize DB: {e}"))?;
        Ok(conn)
    }

    /// Search events by term.
    pub fn search(conn: &Connection, term: &str) -> Result<Vec<EventRow>, String> {
        let mut stmt = conn
            .prepare(
                "SELECT id, timestamp, source, content, preview FROM events
                 WHERE content LIKE ?1 OR preview LIKE ?1
                 ORDER BY timestamp DESC",
            )
            .map_err(|e| format!("Failed to prepare search query: {e}"))?;

        let events = stmt
            .query_map(params![format!("%{term}%")], |row| {
                Ok(EventRow {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    source: row.get(2)?,
                    content: row.get(3)?,
                    preview: row.get(4)?,
                })
            })
            .map_err(|e| format!("Failed to execute search query: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(events)
    }

    /// Get recent events.
    pub fn recent(conn: &Connection, limit: Option<i64>) -> Result<Vec<EventRow>, String> {
        let limit = limit.unwrap_or(50);
        let mut stmt = conn
            .prepare(
                "SELECT id, timestamp, source, content, preview FROM events
                 ORDER BY timestamp DESC LIMIT ?1",
            )
            .map_err(|e| format!("Failed to prepare recent query: {e}"))?;

        let events = stmt
            .query_map(params![limit], |row| {
                Ok(EventRow {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    source: row.get(2)?,
                    content: row.get(3)?,
                    preview: row.get(4)?,
                })
            })
            .map_err(|e| format!("Failed to execute recent query: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(events)
    }

    /// Get events by source.
    pub fn by_source(
        conn: &Connection,
        source: &str,
        limit: Option<i64>,
    ) -> Result<Vec<EventRow>, String> {
        let limit = limit.unwrap_or(50);
        let mut stmt = conn
            .prepare(
                "SELECT id, timestamp, source, content, preview FROM events
                 WHERE source LIKE ?1
                 ORDER BY timestamp DESC LIMIT ?2",
            )
            .map_err(|e| format!("Failed to prepare by_source query: {e}"))?;

        let events = stmt
            .query_map(params![format!("%{source}%"), limit], |row| {
                Ok(EventRow {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    source: row.get(2)?,
                    content: row.get(3)?,
                    preview: row.get(4)?,
                })
            })
            .map_err(|e| format!("Failed to execute by_source query: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect results: {e}"))?;

        Ok(events)
    }

    /// A single event row from the database.
    #[derive(Debug, Clone)]
    pub struct EventRow {
        pub id: i64,
        pub timestamp: String,
        pub source: String,
        pub content: String,
        pub preview: String,
    }

    impl EventRow {
        /// Format the timestamp for display.
        pub fn format_time(&self) -> String {
            // Try RFC3339 parsing, fallback to raw string
            chrono::DateTime::parse_from_rfc3339(&self.timestamp)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|_| self.timestamp.clone())
        }

        /// Get a preview of the content.
        pub fn preview_content(&self, max_len: usize) -> String {
            if self.preview.is_empty() {
                if self.content.len() <= max_len {
                    self.content.clone()
                } else {
                    format!("{}...", &self.content[..max_len])
                }
            } else {
                self.preview.clone()
            }
        }
    }
}

/// Handler for running a command and streaming its output via SSE.
async fn handle_run(
    Json(payload): Json<RunRequest>,
) -> Sse<impl stream::Stream<Item = Result<SseEvent, Infallible>>> {
    let (tx, rx) = mpsc::channel::<AcpResponse>(100);
    let tool = payload.tool;
    let args = payload.args;

    // Spawn a task to manage the process execution and pipe output to the channel.
    tokio::spawn(async move {
        info!("Executing command: {} with args: {:?}", tool, args);

        let mut child = match Command::new(&tool)
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                let _ = tx
                    .send(AcpResponse::Error {
                        error: e.to_string(),
                    })
                    .await;
                return;
            }
        };

        let stdout = child.stdout.take().expect("Failed to take stdout");
        let stderr = child.stderr.take().expect("Failed to take stderr");

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        // Monitor stdout, stderr, and the process exit status.
        loop {
            tokio::select! {
                line = stdout_reader.next_line() => {
                    match line {
                        Ok(Some(l)) => {
                            let _ = tx.send(AcpResponse::Output { data: l }).await;
                        }
                        Ok(None) => {}, // EOF for stdout
                        Err(e) => {
                            let _ = tx.send(AcpResponse::Error { error: e.to_string() }).await;
                            break;
                        }
                    }
                }
                line = stderr_reader.next_line() => {
                    match line {
                        Ok(Some(l)) => {
                            let _ = tx.send(AcpResponse::Output { data: l }).await;
                        }
                        Ok(None) => {}, // EOF for stderr
                        Err(e) => {
                            let _ = tx.send(AcpResponse::Error { error: e.to_string() }).await;
                            break;
                        }
                    }
                }
                status = child.wait() => {
                    match status {
                        Ok(exit_status) => {
                            let _ = tx.send(AcpResponse::Done {
                                status: format!("Exit code: {}", exit_status)
                            }).await;
                        }
                        Err(e) => {
                            let _ = tx.send(AcpResponse::Error { error: e.to_string() }).await;
                        }
                    }
                    break;
                }
            }
        }
    });

    // Convert the mpsc receiver into an SSE stream.
    let stream = stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Some(response) => {
                let data = serde_json::to_string(&response).unwrap_or_default();
                Some((Ok(SseEvent::default().data(data)), rx))
            }
            None => None, // Stream ends when the channel is closed.
        }
    });

    Sse::new(stream)
}

/// Handler for prompt requests (standard JSON response).
async fn handle_prompt(Json(payload): Json<PromptRequest>) -> Json<AcpResponse> {
    info!("Received prompt: {}", payload.message);
    Json(AcpResponse::Input {
        message: format!("Acknowledged prompt: {}", payload.message),
    })
}

/// Handler for chat requests that queries the LLM via LM Studio.
async fn handle_chat(
    Json(payload): Json<ChatRequest>,
) -> Result<Json<AcpResponse>, axum::http::StatusCode> {
    let model = payload.model.unwrap_or_else(|| "local-model".to_string());
    let lm_studio_url = std::env::var("LM_STUDIO_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:1234/v1/chat/completions".to_string());

    info!(
        "Chat request received. Model: {}, URL: {}",
        model, lm_studio_url
    );

    let client = reqwest::Client::new();
    let chat_request = ChatCompletionRequest {
        model,
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: payload.prompt,
        }],
    };

    let response = client
        .post(&lm_studio_url)
        .json(&chat_request)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to connect to LM Studio: {}", e);
            axum::http::StatusCode::BAD_GATEWAY
        })?;

    let chat_response = response
        .json::<ChatCompletionResponse>()
        .await
        .map_err(|e| {
            error!("Failed to parse LM Studio response: {}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if let Some(choice) = chat_response.choices.first() {
        // Check if the LLM wants to call a tool
        if let Some(tool_calls) = &choice.tool_calls {
            if !tool_calls.is_empty() {
                // Collect all tool call results
                let mut results = Vec::new();
                for tool_call in tool_calls {
                    info!(
                        "LLM requested tool call: {} with args: {}",
                        tool_call.function.name, tool_call.function.arguments
                    );

                    // Try to parse the arguments as JSON
                    let args: Vec<String> =
                        match serde_json::from_str(&tool_call.function.arguments) {
                            Ok(parsed) => parsed,
                            Err(e) => {
                                error!("Failed to parse tool arguments: {}", e);
                                results.push(format!(
                                    "Error parsing arguments for {}: {}",
                                    tool_call.function.name, e
                                ));
                                continue;
                            }
                        };

                    // Execute the tool call
                    let tool_result = execute_tool_call(&tool_call.function.name, &args).await;
                    results.push(format!(
                        "Tool '{}' executed: {}",
                        tool_call.function.name, tool_result
                    ));
                }

                // Return the combined results
                let output = results.join("\n");
                Ok(Json(AcpResponse::Output { data: output }))
            } else {
                // No tool calls, return the content
                let content = choice.message.content.clone();
                info!("LLM Response: {}", content);
                Ok(Json(AcpResponse::Output { data: content }))
            }
        } else {
            // No tool_calls field, return the content
            let content = choice.message.content.clone();
            info!("LLM Response: {}", content);
            Ok(Json(AcpResponse::Output { data: content }))
        }
    } else {
        Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
    }
}

/// Execute a tool call based on the function name and arguments.
async fn execute_tool_call(function_name: &str, args: &[String]) -> String {
    match function_name {
        "run_command" => {
            if args.len() < 2 {
                return "Error: run_command requires at least 2 arguments (tool and args)"
                    .to_string();
            }
            let tool = &args[0];
            let command_args = &args[1..];

            // Security layer: check command before execution.
            let guard_root = std::env::var("MIRROR_GUARD_ROOT")
                .unwrap_or_else(|_| "/home/crombo/mirror-lab".to_string());

            let guard_db = mirror_guard::GuardDb::open(mirror_guard::GuardDb::from_mirror_path(
                format!("{}/mirror.db", &guard_root),
            ))
            .unwrap_or_else(|_| {
                warn!("Failed to open guard DB, using in-memory fallback");
                mirror_guard::GuardDb::open(":memory:").unwrap()
            });

            let gate = ExecutionGate::new(&guard_db, false, &guard_root);

            let mut concierge = concierge::GateConcierge::new();

            match gate.check(GateContext {
                action_type: "tool_call",
                command: tool,
                args: command_args.to_vec(),
                trust_layer: 2,
                confidence: mirror_guard::TrustScore::new(0.5),
                has_raw_data: true,
                has_uncertainty: true,
                can_interrupt: true,
            }) {
                Ok(result) => {
                    let (status, pending_entry, interrupted_entry) =
                        concierge.enforce(result, "tool_call", tool, command_args, 2, 0.5);

                    match status {
                        ActionStatus::Approved => {
                            info!(
                                "Gate concierge: Proceed — {} with args {:?}",
                                tool, command_args
                            );
                        }
                        ActionStatus::Pending => {
                            if let Some(ref entry) = pending_entry {
                                info!(
                                    gate_result_id = %entry.gate_result_id,
                                    pending_id = %entry.id,
                                    "Gate concierge: Pending → PendingQueue — queued for review"
                                );
                            }
                            return format!(
                                "Pending: queued for review (pending_id: {})",
                                pending_entry
                                    .as_ref()
                                    .map(|e| e.id.clone())
                                    .unwrap_or_default()
                            );
                        }
                        ActionStatus::Denied => {
                            if let Some(ref entry) = interrupted_entry {
                                info!(
                                    gate_result_id = %entry.gate_result_id,
                                    interrupted_id = %entry.id,
                                    reason = %entry.reason,
                                    "Gate concierge: Interrupted → InterruptedLog"
                                );
                            }
                            return format!(
                                "Interrupted: {} (interrupted_id: {})",
                                interrupted_entry
                                    .as_ref()
                                    .map(|e| e.reason.clone())
                                    .unwrap_or_default(),
                                interrupted_entry
                                    .as_ref()
                                    .map(|e| e.id.clone())
                                    .unwrap_or_default()
                            );
                        }
                        ActionStatus::Executed | ActionStatus::Interrupted => {
                            return "Status not handled by concierge".to_string();
                        }
                    }
                }
                Err(e) => {
                    error!("Security gate error: {}", e);
                    return format!("Security gate error: {}", e);
                }
            }

            info!(
                "Executing tool call: {} with args: {:?}",
                tool, command_args
            );

            let mut child = match Command::new(tool)
                .args(command_args)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(e) => {
                    return format!("Error spawning command: {}", e);
                }
            };

            let stdout = child.stdout.take().expect("Failed to take stdout");
            let stderr = child.stderr.take().expect("Failed to take stderr");

            let mut stdout_reader = BufReader::new(stdout).lines();
            let mut stderr_reader = BufReader::new(stderr).lines();

            let mut output = String::new();

            loop {
                tokio::select! {
                    line = stdout_reader.next_line() => {
                        match line {
                            Ok(Some(l)) => {
                                output.push_str(&l);
                                output.push('\n');
                            }
                            Ok(None) => {},
                            Err(e) => {
                                output.push_str(&format!("Error reading stdout: {}", e));
                                break;
                            }
                        }
                    }
                    line = stderr_reader.next_line() => {
                        match line {
                            Ok(Some(l)) => {
                                output.push_str(&format!("stderr: {}\n", l));
                            }
                            Ok(None) => {},
                            Err(e) => {
                                output.push_str(&format!("Error reading stderr: {}", e));
                                break;
                            }
                        }
                    }
                    status = child.wait() => {
                        match status {
                            Ok(exit_status) => {
                                output.push_str(&format!("\nExit code: {}", exit_status));
                            }
                            Err(e) => {
                                output.push_str(&format!("\nError waiting for process: {}", e));
                            }
                        }
                        break;
                    }
                }
            }

            output
        }
        "search_logs" => {
            let search_req: SearchLogsRequest =
                match serde_json::from_str(&serde_json::to_string(args).unwrap_or_default()) {
                    Ok(req) => req,
                    Err(e) => {
                        return format!("Error parsing search_logs arguments: {}", e);
                    }
                };

            let db_path = std::env::var("MIRROR_LOG_DB_PATH")
                .unwrap_or_else(|_| "/home/crombo/mirror-lab/mirror-log/mirror.db".to_string());

            let conn = match local_log::init_db(&db_path) {
                Ok(conn) => conn,
                Err(e) => {
                    return format!("Error initializing event database: {}", e);
                }
            };

            let events = match local_log::search(&conn, &search_req.term) {
                Ok(events) => events,
                Err(e) => {
                    return format!("Error searching events: {}", e);
                }
            };

            let mut output = String::new();
            output.push_str(&format!(
                "Found {} events matching '{}':\n",
                events.len(),
                search_req.term
            ));

            for event in events.iter().take(search_req.limit.unwrap_or(10) as usize) {
                output.push_str(&format!(
                    "[{}] {} - {}\n  Content: {}\n",
                    event.format_time(),
                    event.source,
                    event.id,
                    event.preview_content(200)
                ));
            }

            output
        }
        "recent_events" => {
            let recent_req: RecentEventsRequest =
                match serde_json::from_str(&serde_json::to_string(args).unwrap_or_default()) {
                    Ok(req) => req,
                    Err(e) => {
                        return format!("Error parsing recent_events arguments: {}", e);
                    }
                };

            let db_path = std::env::var("MIRROR_LOG_DB_PATH")
                .unwrap_or_else(|_| "/home/crombo/mirror-lab/mirror-log/mirror.db".to_string());

            let conn = match local_log::init_db(&db_path) {
                Ok(conn) => conn,
                Err(e) => {
                    return format!("Error initializing event database: {}", e);
                }
            };

            let events = match local_log::recent(&conn, Some(recent_req.limit)) {
                Ok(events) => events,
                Err(e) => {
                    return format!("Error fetching recent events: {}", e);
                }
            };

            let mut output = String::new();
            output.push_str(&format!("Recent {} events:\n", events.len()));

            for event in events.iter() {
                output.push_str(&format!(
                    "[{}] {} - {}\n  Content: {}\n",
                    event.format_time(),
                    event.source,
                    event.id,
                    event.preview_content(200)
                ));
            }

            output
        }
        "by_source" => {
            let source_req: BySourceRequest =
                match serde_json::from_str(&serde_json::to_string(args).unwrap_or_default()) {
                    Ok(req) => req,
                    Err(e) => {
                        return format!("Error parsing by_source arguments: {}", e);
                    }
                };

            let db_path = std::env::var("MIRROR_LOG_DB_PATH")
                .unwrap_or_else(|_| "/home/crombo/mirror-lab/mirror-log/mirror.db".to_string());

            let conn = match local_log::init_db(&db_path) {
                Ok(conn) => conn,
                Err(e) => {
                    return format!("Error initializing event database: {}", e);
                }
            };

            let events = match local_log::by_source(&conn, &source_req.source, source_req.limit) {
                Ok(events) => events,
                Err(e) => {
                    return format!("Error fetching events by source: {}", e);
                }
            };

            let mut output = String::new();
            output.push_str(&format!("Events from source '{}':\n", source_req.source));

            for event in events.iter() {
                output.push_str(&format!(
                    "[{}] {} - {}\n  Content: {}\n",
                    event.format_time(),
                    event.source,
                    event.id,
                    event.preview_content(200)
                ));
            }

            output
        }
        _ => format!("Unknown tool: {}", function_name),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for structured logging.
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    // Define the Axum router with SSE and JSON endpoints.
    let app = Router::new()
        .route("/acp/run", post(handle_run))
        .route("/acp/prompt", post(handle_prompt))
        .route("/acp/chat", post(handle_chat))
        .layer(CorsLayer::permissive());

    // Bind to localhost:3000.
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("ACP Orchestrator listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
