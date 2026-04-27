use serde::Serialize;
use serde_json::json;
use thiserror::Error;

use chrono::Datelike;

#[derive(Error, Debug)]
pub enum Error {
    #[error("session file not found")]
    SessionNotFound(String),
    #[error("sqlite read failed: {0}")]
    SqliteRead(String),
    #[error("provider not discovered")]
    ProviderNotFound(String),
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("utf8 error")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("json parse error")]
    Json(#[from] serde_json::Error),
    #[error("sqlite error")]
    Sqlite(#[from] rusqlite::Error),
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionData {
    pub provider: String,
    pub date: chrono::NaiveDateTime,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub model: String,
    pub task_category: String,
    pub project: Option<String>,
    pub message_id: Option<String>,
}

#[derive(Debug)]
pub struct ProviderRegistry {
    providers: Vec<Provider>,
}

#[derive(Debug, Clone)]
pub struct Provider {
    pub name: String,
    pub data_path: std::path::PathBuf,
    pub format: DataFormat,
}

#[derive(Debug, Clone)]
pub enum DataFormat {
    Jsonl,
    Sqlite,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn discover(project_root: &std::path::Path) -> Result<Vec<Provider>, Error> {
        let mut providers = Vec::new();

        // Claude Code
        let claude_path = project_root.join("~/.claude/projects/");
        if claude_path.exists() {
            providers.push(Provider {
                name: "claude_code".to_string(),
                data_path: claude_path,
                format: DataFormat::Jsonl,
            });
        }

        // Claude Desktop
        let claude_desktop_path =
            project_root.join("~/Library/Application Support/Claude/local-agent-mode-sessions/");
        if claude_desktop_path.exists() {
            providers.push(Provider {
                name: "claude_desktop".to_string(),
                data_path: claude_desktop_path,
                format: DataFormat::Jsonl,
            });
        }

        // Codex
        let codex_path = project_root.join("~/.codex/sessions/");
        if codex_path.exists() {
            providers.push(Provider {
                name: "codex".to_string(),
                data_path: codex_path,
                format: DataFormat::Jsonl,
            });
        }

        // Cursor
        let cursor_path = project_root
            .join("~/Library/Application Support/Cursor/User/globalStorage/state.vscdb");
        if cursor_path.exists() {
            providers.push(Provider {
                name: "cursor".to_string(),
                data_path: cursor_path,
                format: DataFormat::Sqlite,
            });
        }

        // OpenCode
        let opencode_path = project_root.join("~/.local/share/opencode/opencode*.db");
        if opencode_path.exists() {
            providers.push(Provider {
                name: "opencode".to_string(),
                data_path: opencode_path,
                format: DataFormat::Sqlite,
            });
        }

        // Pi
        let pi_path = project_root.join("~/.pi/agent/sessions/");
        if pi_path.exists() {
            providers.push(Provider {
                name: "pi".to_string(),
                data_path: pi_path,
                format: DataFormat::Jsonl,
            });
        }

        // OMP
        let omp_path = project_root.join("~/.omp/agent/sessions/");
        if omp_path.exists() {
            providers.push(Provider {
                name: "omp".to_string(),
                data_path: omp_path,
                format: DataFormat::Jsonl,
            });
        }

        // GitHub Copilot
        let copilot_path = project_root.join("~/.copilot/session-state/");
        if copilot_path.exists() {
            providers.push(Provider {
                name: "copilot".to_string(),
                data_path: copilot_path,
                format: DataFormat::Jsonl,
            });
        }

        Ok(providers)
    }

    pub fn read_sessions(&self) -> Result<Vec<SessionData>, Error> {
        let mut sessions = Vec::new();

        for provider in &self.providers {
            match provider.format {
                DataFormat::Jsonl => {
                    let jsonl_sessions = read_jsonl(&provider.data_path)?;
                    sessions.extend(jsonl_sessions);
                }
                DataFormat::Sqlite => {
                    let sqlite_sessions = read_sqlite(&provider.data_path)?;
                    sessions.extend(sqlite_sessions);
                }
            }
        }

        Ok(sessions)
    }

    pub fn today_usage(&self) -> Result<serde_json::Value, Error> {
        let today = chrono::Local::now().date_naive();
        let sessions = self.read_sessions()?;
        let today_sessions: Vec<SessionData> = sessions
            .into_iter()
            .filter(|s| s.date.date() == today)
            .collect();

        Ok(json!({
            "total": today_sessions.iter().map(|s| s.output_tokens).sum::<u64>(),
            "sessions": today_sessions.len(),
        }))
    }

    pub fn month_usage(&self) -> Result<serde_json::Value, Error> {
        let month = chrono::Local::now().date_naive().month();
        let sessions = self.read_sessions()?;
        let month_sessions: Vec<SessionData> = sessions
            .into_iter()
            .filter(|s| s.date.month() == month)
            .collect();

        Ok(json!({
            "total": month_sessions.iter().map(|s| s.output_tokens).sum::<u64>(),
            "sessions": month_sessions.len(),
        }))
    }

    pub fn multi_period_export(&self) -> Result<serde_json::Value, Error> {
        let sessions = self.read_sessions()?;

        Ok(json!({
            "periods": {
                "today": sessions.len(),
                "7_days": sessions.len(),
                "30_days": sessions.len(),
            },
        }))
    }

    pub fn provider_sessions(&self, name: &str) -> Result<Vec<SessionData>, Error> {
        let sessions = self.read_sessions()?;
        let filtered = sessions
            .into_iter()
            .filter(|s| s.provider == name)
            .collect();

        Ok(filtered)
    }
}

fn read_jsonl(path: &std::path::Path) -> Result<Vec<SessionData>, Error> {
    let content = std::fs::read(path)?;

    if content.len() > 128_000_000 {
        return Err(Error::SessionNotFound("file exceeds 128 MB bound".into()));
    }

    let lines = std::str::from_utf8(&content)?
        .lines()
        .map(serde_json::from_str::<serde_json::Value>)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(lines
        .iter()
        .map(|v| SessionData {
            provider: v
                .get("provider")
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_string(),
            date: v
                .get("date")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .parse()
                .unwrap_or(chrono::NaiveDateTime::default()),
            input_tokens: v.get("input_tokens").and_then(|i| i.as_u64()).unwrap_or(0),
            output_tokens: v.get("output_tokens").and_then(|o| o.as_u64()).unwrap_or(0),
            model: v
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string(),
            task_category: v
                .get("task_category")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string(),
            project: v.get("project").and_then(|p| p.as_str()).map(String::from),
            message_id: v
                .get("message_id")
                .and_then(|m| m.as_str())
                .map(String::from),
        })
        .collect())
}

fn read_sqlite(path: &std::path::Path) -> Result<Vec<SessionData>, Error> {
    let conn = rusqlite::Connection::open(path)?;
    let mut stmt = conn.prepare("SELECT provider, date, input_tokens, output_tokens, model, project, message_id FROM sessions")?;

    let mut sessions = Vec::new();

    let rows = stmt.query_map([], |row| {
        Ok(SessionData {
            provider: row.get(0)?,
            date: row.get::<usize, String>(1)?.parse().expect("date parse"),
            input_tokens: row.get(2)?,
            output_tokens: row.get(3)?,
            model: row.get(4)?,
            task_category: "".to_string(),
            project: row.get::<usize, String>(5).ok(),
            message_id: row.get::<usize, String>(6).ok(),
        })
    })?;

    for row in rows {
        sessions.push(row?);
    }

    Ok(sessions)
}
