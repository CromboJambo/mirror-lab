use anyhow::{Context, Result};
use clap::Parser;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Mirror-Query - Local AI decompression layer for mirror-log
///
/// Query your append-only event log with natural language using local AI.
/// Supports both recent-event retrieval and semantic similarity search via Ollama embeddings.
#[derive(Parser)]
#[command(name = "mirror-query")]
#[command(about = "Local AI query interface for mirror-log", long_about = None)]
struct Cli {
    /// Natural language query
    query: String,

    /// Mode of retrieval: 'recent' (latest N events) or 'imprint' (semantic similarity search via embeddings)
    #[arg(short, long, default_value = "recent")]
    mode: String,

    /// Number of events to retrieve (for recent) or top-K similarity matches (for semantic)
    #[arg(short, long, default_value = "50")]
    limit: i64,

    /// Path to mirror.db
    #[arg(short, long, default_value = "mirror.db")]
    db: PathBuf,

    /// Filter by source
    #[arg(short, long)]
    source: Option<String>,

    /// Ollama API endpoint
    #[arg(long, default_value = "http://localhost:11434")]
    ollama_url: String,

    /// Ollama model to use (for text generation)
    #[arg(short, long, default_value = "llama3.2")]
    model: String,

    /// Output format (text, json, markdown)
    #[arg(short, long, default_value = "text")]
    format: String,

    /// Show debug information
    #[arg(long)]
    debug: bool,

    /// HTTP request timeout in seconds (default: 60)
    #[arg(long, default_value = "60")]
    timeout: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Event {
    id: String,
    timestamp: i64,
    source: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<String>,
}

impl Event {
    fn format_time(&self) -> String {
        use chrono::Utc;
        match chrono::DateTime::<Utc>::from_timestamp(self.timestamp, 0) {
            Some(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            None => format!("epoch:{}", self.timestamp),
        }
    }
}

#[derive(Debug, Serialize)]
struct QueryResult {
    query: String,
    mode: String,
    response: String,
    events_analyzed: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.debug {
        eprintln!("🔍 Debug mode enabled");
        eprintln!("Mode: {}", cli.mode);
        eprintln!("Database: {:?}", cli.db);
        eprintln!("Query: {}", cli.query);
        eprintln!("Ollama URL: {}", cli.ollama_url);
    }

    let conn = Connection::open(&cli.db)
        .with_context(|| format!("Failed to open database: {:?}", cli.db))?;

    // 1. Retrieval Stage
    let events = if cli.mode == "semantic" || cli.mode == "imprint" {
        if cli.debug {
            eprintln!("🧬 Performing semantic similarity search...");
        }
        perform_semantic_search(&conn, &cli, &cli.ollama_url)?
    } else {
        if cli.debug {
            eprintln!("🕒 Fetching most recent events...");
        }
        fetch_recent_events(&conn, cli.limit, cli.source.as_deref())?
    };

    if events.is_empty() {
        eprintln!("⚠️  No relevant events found in database.");
        return Ok(());
    }

    // 2. Context Engineering (RAG)
    let context = build_context(&events);

    if cli.debug {
        eprintln!(
            "Context size: {} chars\nSending to Ollama...",
            context.len()
        );
    }

    // 3. Generation Stage
    let response = query_ollama(
        &cli.ollama_url,
        &cli.model,
        &cli.query,
        &context,
        cli.debug,
        cli.timeout,
    )?;

    // 4. Presentation Stage
    match cli.format.as_str() {
        "json" => {
            let output = QueryResult {
                query: cli.query,
                mode: cli.mode,
                response,
                events_analyzed: events.len(),
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        "markdown" => {
            println!("# Query\n\n{}\n", cli.query);
            println!("## Response\n\n{}\n", response);
            println!(
                "---\n*Analyzed {} events using {} mode*",
                events.len(),
                cli.mode
            );
        }
        _ => {
            println!("{}", response);
            if cli.debug {
                eprintln!("\n📈 Stats: Analyzed {} events", events.len());
            }
        }
    }

    Ok(())
}

// Helper for debug printing since we don't have a logger set up in main
#[macro_export]
macro_rules! eprust_info {
    ($($arg:tt)*) => { eprintln!($($arg)*); };
}

fn fetch_recent_events(conn: &Connection, limit: i64, source: Option<&str>) -> Result<Vec<Event>> {
    let query = if source.is_some() {
        "SELECT id, timestamp, source, content, meta FROM events WHERE source = ?1 ORDER BY timestamp DESC LIMIT ?2".to_string()
    } else {
        "SELECT id, timestamp, source, content, meta FROM events ORDER BY timestamp DESC LIMIT ?1"
            .to_string()
    };

    let mut stmt = conn.prepare(&query)?;
    let events_iter = if let Some(s) = source {
        stmt.query_map(params![s, limit], map_event_row)?
    } else {
        stmt.query_map(params![limit], map_event_row)?
    };

    let mut events = Vec::new();
    for event in events_iter {
        events.push(event?);
    }

    Ok(events)
}

fn fetch_single_event(conn: &Connection, id: &str) -> Result<Event> {
    conn.query_row(
        "SELECT id, timestamp, source, content, meta FROM events WHERE id = ?1",
        [id],
        map_event_row,
    )
    .map_err(|e| anyhow::anyhow!(e))
}

fn map_event_row(row: &rusqlite::Row) -> rusqlite::Result<Event> {
    Ok(Event {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        source: row.get(2)?,
        content: row.get(3)?,
        meta: row.get(4).ok(),
    })
}

fn perform_semantic_search(conn: &Connection, cli: &Cli, ollama_url: &str) -> Result<Vec<Event>> {
    let query_embedding = get_ollama_embedding(ollama_url, &cli.query, cli.timeout)?;

    if cli.debug {
        eprintln!("✓ Query embedded successfully");
    }

    // Fetch all embeddings and their associated event IDs from the DB
    let mut stmt = conn.prepare("SELECT event_id, embedding FROM event_embeddings")?;
    let embedding_rows = stmt.query_map([], |row| {
        let event_id: String = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;
        // Convert bytes back to f32 vector (Little Endian)
        let vec = blob
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect::<Vec<f32>>();
        Ok((event_id, vec))
    })?;

    let mut scored_events = Vec::new();
    for row in embedding_rows {
        let (event_id, event_vec) = row?;
        let score = cosine_similarity(&query_embedding, &event_vec);
        scored_events.push((score, event_id));
    }

    // Sort by highest similarity and take Top-K
    scored_events.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let top_k = scored_events
        .into_iter()
        .take(cli.limit as usize)
        .collect::<Vec<_>>();

    if cli.debug {
        eprintln!("✓ Found {} matches via vector similarity", top_k.len());
    }

    let mut final_events = Vec::new();
    for (_, id) in top_k {
        if let Ok(event) = fetch_single_event(conn, &id) {
            final_events.push(event);
        }
    }

    Ok(final_events)
}

fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    let dot: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
    let norm1: f32 = v1.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm2: f32 = v2.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm1 == 0.0 || norm2 == 0.0 {
        0.0
    } else {
        dot / (norm1 * norm2)
    }
}

fn build_context(events: &[Event]) -> String {
    let mut context =
        String::from("Here are relevant entries from the user's append-only log:\n\n");
    for event in events.iter().rev() {
        context.push_str(&format!(
            "[{}] [{}] {}\n",
            event.format_time(),
            event.source,
            event.content
        ));
        if let Some(meta) = &event.meta {
            context.push_str(&format!("  Meta: {}\n", meta));
        }
        context.push('\n');
    }
    context
}

fn get_ollama_embedding(url: &str, text: &str, timeout: u64) -> Result<Vec<f32>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout))
        .build()?;

    // Using the standard Ollama /api/embeddings endpoint
    let request = serde_json::json!({
        "model": "llama3.2",
        "prompt": text,
    });

    let response = client
        .post(format!("{}/api/embeddings", url))
        .json(&request)
        .send()
        .context("Failed to connect to Ollama embedding endpoint")?;

    let json: serde_json::Value = response.json()?;
    let embedding = json["embedding"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No embedding found in response"))?
        .iter()
        .filter_map(|v| v.as_f64().map(|f| f as f32))
        .collect();

    Ok(embedding)
}

fn query_ollama(
    url: &str,
    model: &str,
    user_query: &str,
    context: &str,
    debug: bool,
    timeout: u64,
) -> Result<String> {
    let prompt = format!(
        "{}\n\n---\n\nUser question: {}\n\nProvide a helpful, concise answer based on the entries above.",
        context, user_query
    );

    if debug {
        eprintln!("\n📤 Prompt length: {} chars", prompt.len());
    }

    let request = OllamaRequest {
        model: model.to_string(),
        prompt,
        stream: false,
    };

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout))
        .build()?;

    let response = client
        .post(format!("{}/api/generate", url))
        .json(&request)
        .send()
        .context("Failed to connect to Ollama. Is it running? Try: ollama serve")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        anyhow::bail!("Ollama API error ({}): {}", status, body);
    }

    let ollama_response: OllamaResponse =
        response.json().context("Failed to parse Ollama response")?;

    Ok(ollama_response.response.trim().to_string())
}
