use crate::db::Database;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Represents a config change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChange {
    pub timestamp: String,
    pub tool: String,
    pub file_path: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub change_type: String,
    pub outcome: String,
    pub user_context: Option<String>,
    pub metadata: ChangeMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeMetadata {
    pub keys_changed: Vec<String>,
    pub change_scope: String,
    pub tool_version: Option<String>,
}

pub struct ConfigObserver {
    pub db: Database,
    pub watched_files: Vec<WatchedFile>,
}

#[derive(Debug, Clone)]
pub struct WatchedFile {
    pub path: String,
    pub tool: String,
    pub file_type: FileType,
}

#[derive(Debug, Clone)]
pub enum FileType {
    Lua, Kdl, Nu, Json, Toml, Shell, Other,
}

impl ConfigObserver {
    pub fn new(db: Database) -> Result<Self> {
        let watched_files = Self::get_default_watched_files();
        Ok(Self { db, watched_files })
    }

    fn get_default_watched_files() -> Vec<WatchedFile> {
        vec![
            WatchedFile { path: "~/.config/wezterm/wezterm.lua".to_string(), tool: "wezterm".to_string(), file_type: FileType::Lua },
            WatchedFile { path: "~/.config/zellij/config.kdl".to_string(), tool: "zellij".to_string(), file_type: FileType::Kdl },
            WatchedFile { path: "~/.config/nushell/config.nu".to_string(), tool: "nushell".to_string(), file_type: FileType::Nu },
            WatchedFile { path: "~/.config/nushell/env.nu".to_string(), tool: "nushell".to_string(), file_type: FileType::Nu },
            WatchedFile { path: "~/.config/zed/settings.json".to_string(), tool: "zed".to_string(), file_type: FileType::Json },
        ]
    }

    pub fn watch(&self) -> Result<()> {
        info!("Starting config observation for {} files", self.watched_files.len());
        Ok(())
    }

    pub fn log_change(&self, change: ConfigChange) -> Result<()> {
        self.db.log_config_change(change)
    }

    pub fn detect_change(&self, file: &WatchedFile) -> Result<Option<ConfigChange>> {
        Ok(None)
    }
}

impl FileType {
    pub fn from_path(path: &str) -> Self {
        let path_lower = path.to_lowercase();
        if path_lower.ends_with(".lua") { FileType::Lua }
        else if path_lower.ends_with(".kdl") { FileType::Kdl }
        else if path_lower.ends_with(".nu") { FileType::Nu }
        else if path_lower.ends_with(".json") { FileType::Json }
        else if path_lower.ends_with(".toml") { FileType::Toml }
        else if path_lower.ends_with(".sh") || path_lower.ends_with(".bash") { FileType::Shell }
        else { FileType::Other }
    }
}
