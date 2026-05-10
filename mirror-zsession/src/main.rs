use anyhow::Result;
use chrono::Duration;
use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "mirror-zsession")]
#[command(about = "List and connect to Zellij sessions via WebSocket")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    List {
        /// Generate a new auth token and insert into tokens.db
        #[arg(long)]
        create_token: bool,
    },
    Connect {
        /// Zellij session name to connect to
        session: String,
        /// Auth token (or ZELLIJ_AUTH_TOKEN env var)
        #[arg(long)]
        auth_token: Option<String>,
    },
}

fn get_zellij_dirs() -> anyhow::Result<(PathBuf, PathBuf)> {
    let dirs: ProjectDirs = match ProjectDirs::from("org", "Zellij Contributors", "Zellij") {
        Some(d) => d,
        None => anyhow::bail!("Zellij ProjectDirs not available"),
    };

    let sock_dir = dirs
        .runtime_dir()
        .ok_or_else(|| anyhow::anyhow!("Zellij runtime_dir not available"))?
        .to_owned();

    let cache_base = dirs.cache_dir().to_owned();
    let cache_dir = cache_base.join("contract_version_1").join("session_info");

    Ok((sock_dir, cache_dir))
}

fn get_token_db_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("org", "Zellij Contributors", "Zellij")
        .ok_or_else(|| anyhow::anyhow!("Zellij ProjectDirs not available"))?;

    let data_dir = dirs.data_dir().to_owned();

    Ok(data_dir.join("tokens.db"))
}

fn list_active_sessions(sock_dir: &PathBuf) -> Result<Vec<Value>> {
    let mut sessions = Vec::new();

    let entries = fs::read_dir(sock_dir)?;

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().into_string().unwrap();

        let path = entry.path();

        let metadata = fs::metadata(&path)?;
        let modified = metadata.modified()?;
        let elapsed = modified.elapsed()?;
        let age = Duration::from_std(elapsed)?;

        let is_socket = entry.file_type()?.is_socket();

        let status = if is_socket { "active" } else { "exit" };

        sessions.push(json!({"name": name, "age": age.to_string(), "status": status}));
    }

    Ok(sessions)
}

fn list_resurrectable_sessions(cache_dir: &PathBuf) -> Result<Vec<Value>> {
    let mut sessions = Vec::new();

    let entries = fs::read_dir(cache_dir)?;

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

fn validate_auth_token(auth_token: &str, db_path: &PathBuf) -> Result<bool> {
    let token_hash = {
        let mut hasher = Sha256::new();
        hasher.update(auth_token.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    let conn = rusqlite::Connection::open(db_path)?;

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tokens WHERE token_hash = ?1",
        [&token_hash],
        |row| row.get(0),
    )?;

    Ok(count > 0)
}

fn create_session_token(auth_token: &str, db_path: &PathBuf) -> Result<String> {
    let token_hash = {
        let mut hasher = Sha256::new();
        hasher.update(auth_token.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    let conn = rusqlite::Connection::open(db_path)?;

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

fn create_new_token(db_path: &PathBuf) -> Result<Uuid> {
    let conn = rusqlite::Connection::open(db_path)?;

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM tokens", [], |row| row.get(0))?;

    let token = Uuid::new_v4();
    let token_hash = {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    };
    let name = format!("token_{}", count + 1);

    conn.execute(
        "INSERT INTO tokens (token_hash, name, read_only) VALUES (?1, ?2, ?3)",
        [&token_hash, &name, "0"],
    )?;

    Ok(token)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let (sock_dir, cache_dir) = get_zellij_dirs()?;
    let token_db_path = get_token_db_path()?;

    match cli.command {
        Commands::List { create_token } => {
            if create_token {
                let token = create_new_token(&token_db_path)?;
                let output = json!({"new_token": token.to_string()});
                println!("{}", serde_json::to_string(&output)?);
            }

            let active = list_active_sessions(&sock_dir)?;
            let resurrectable = list_resurrectable_sessions(&cache_dir)?;

            let output = json!({"active": active, "resurrectable": resurrectable});
            println!("{}", serde_json::to_string(&output)?);

            Ok(())
        }
        Commands::Connect {
            session: _,
            auth_token,
        } => {
            let auth_token = auth_token
                .or_else(|| std::env::var("ZELLIJ_AUTH_TOKEN").ok())
                .ok_or_else(|| anyhow::anyhow!("auth token required"))?;

            let valid = validate_auth_token(&auth_token, &token_db_path)?;

            let output = json!({"auth_valid": valid});
            println!("{}", serde_json::to_string(&output)?);

            if valid {
                let session_token = create_session_token(&auth_token, &token_db_path)?;
                let output = json!({"session_token": session_token});
                println!("{}", serde_json::to_string(&output)?);
            }

            Ok(())
        }
    }
}
