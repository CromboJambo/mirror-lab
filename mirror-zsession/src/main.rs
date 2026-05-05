use anyhow::Result;
use chrono::Duration;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::PathBuf;
use uuid::Uuid;

const ZELLIJ_SOCK_DIR: &str = "/home/crombo/.local/share/zellij";
const ZELLIJ_SESSION_INFO_CACHE_DIR: &str = "/home/crombo/.cache/zellij/contract_version_1/session_info";

fn list_active_sessions() -> Result<Vec<Value>> {
    let mut sessions = Vec::new();

    let sock_dir = PathBuf::from(ZELLIJ_SOCK_DIR);
    let entries = fs::read_dir(&sock_dir)?;

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().into_string().unwrap();

        if name == "tokens.db" {
            continue;
        }

        let path = entry.path();

        let metadata = fs::metadata(&path)?;
        let modified = metadata.modified()?;
        let elapsed = modified.elapsed()?;
        let age = Duration::from_std(elapsed)?;

        let is_socket = entry.file_type()?.is_socket();

        let status = if is_socket {
            "active"
        } else {
            "exit"
        };

        sessions.push(json!({"name": name, "age": age.to_string(), "status": status}));
    }

    Ok(sessions)
}

fn list_resurrectable_sessions() -> Result<Vec<Value>> {
    let mut sessions = Vec::new();

    let cache_dir = PathBuf::from(ZELLIJ_SESSION_INFO_CACHE_DIR);
    let entries = fs::read_dir(&cache_dir)?;

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().into_string().unwrap();

        let layout_file = cache_dir.join(&name).join("session-layout.kdl");
        if !layout_file.exists() {
            continue;
        }

        let metadata = fs::metadata(&layout_file)?;
        let modified = metadata.modified()?;
        let elapsed = modified.elapsed()?;
        let age = Duration::from_std(elapsed)?;

        sessions.push(json!({"name": name, "age": age.to_string(), "status": "exit"}));
    }

    Ok(sessions)
}

fn validate_auth_token(auth_token: &str) -> Result<bool> {
    let token_hash = {
        let mut hasher = Sha256::new();
        hasher.update(auth_token.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    let db_path = PathBuf::from("/home/crombo/.local/share/zellij/tokens.db");
    let conn = rusqlite::Connection::open(&db_path)?;

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tokens WHERE token_hash = ?1",
        [&token_hash],
        |row| row.get(0),
    )?;

    Ok(count > 0)
}

fn create_session_token(auth_token: &str) -> Result<String> {
    let token_hash = {
        let mut hasher = Sha256::new();
        hasher.update(auth_token.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    let db_path = PathBuf::from("/home/crombo/.local/share/zellij/tokens.db");
    let conn = rusqlite::Connection::open(&db_path)?;

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tokens WHERE token_hash = ?1",
        [&token_hash],
        |row| row.get(0),
    )?;

    if count == 0 {
        anyhow::bail!("auth token not found in tokens.db");
    }

    let session_token = Uuid::new_v4().to_string();
    let session_token_hash = {
        let mut hasher = Sha256::new();
        hasher.update(session_token.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    conn.execute(
        "INSERT INTO session_tokens (session_token_hash, auth_token_hash, remember_me, expires_at) VALUES (?1, ?2, ?3, datetime('now', '+5 minutes'))",
        [&session_token_hash, &token_hash, "0"],
    )?;

    Ok(session_token)
}

async fn connect_to_terminal_ws(session_name: &str, web_client_id: &str, auth_token: &str) -> Result<()> {
    let session_token = create_session_token(auth_token)?;

    let url = format!("ws://127.0.0.1:8082/ws/{}", session_name);
    let response = reqwest::Client::new()
        .get(&url)
        .query(&[("web_client_id", web_client_id)])
        .header("Cookie", format!("session_token={}", session_token))
        .send()
        .await?;

    if response.status().is_success() {
        let _socket = response.upgrade().await?;
        println!("connected to ws://127.0.0.1:8082/ws/{}", session_name);
        println!("web_client_id: {}", web_client_id);
        println!("session_token: {}", session_token);
    } else {
        anyhow::bail!("connection failed: {}", response.status());
    }

    Ok(())
}

fn main() -> Result<()> {
    let auth_token = std::env::var("ZELLIJ_AUTH_TOKEN")
        .ok()
        .unwrap_or_else(|| {
            let token = Uuid::new_v4().to_string();
            let token_hash = {
                let mut hasher = Sha256::new();
                hasher.update(token.as_bytes());
                format!("{:x}", hasher.finalize())
            };
            let db_path = PathBuf::from("/home/crombo/.local/share/zellij/tokens.db");
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            let count: i64 = conn.query_row("SELECT COUNT(*) FROM tokens", [], |row| row.get(0)).unwrap();
            let name = format!("token_{}", count + 1);
            conn.execute(
                "INSERT INTO tokens (token_hash, name, read_only) VALUES (?1, ?2, ?3)",
                [&token_hash, &name, "0"],
            ).unwrap();
            println!("new auth token generated: {}", token);
            token
        });

    let valid = validate_auth_token(&auth_token)?;
    println!("auth_token valid: {}", valid);

    if valid {
        let session_token = create_session_token(&auth_token)?;
        println!("session_token: {}", session_token);
    }

    let active = list_active_sessions()?;
    let resurrectable = list_resurrectable_sessions()?;

    let output = Value::Object(serde_json::Map::from_iter([
        ("active".to_string(), Value::Array(active)),
        ("resurrectable".to_string(), Value::Array(resurrectable)),
    ]));
    println!("{}", serde_json::to_string(&output)?);

    Ok(())
}
