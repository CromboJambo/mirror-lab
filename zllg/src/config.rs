use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{Result, ZllgError};

/// A registered pane definition in the config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneConfig {
    /// Logical name used to reference this pane in keybinds.
    pub name: String,
    /// Command to run inside the pane.
    pub command: String,
    /// Optional args passed to the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Zellij pane index (used for focus/hide targeting).
    #[serde(default)]
    pub index: usize,
}

/// Root config structure (`~/.config/zllg/config.toml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZllgConfig {
    /// Shell to fall back to when POSIX compatibility is needed.
    #[serde(default = "default_shell")]
    pub default_shell: String,
    /// Registered pane definitions.
    #[serde(default)]
    pub panes: Vec<PaneConfig>,
    /// Optional WezTerm workspace names for monitor mapping.
    #[serde(default)]
    pub workspaces: Vec<WorkspaceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Workspace identifier passed to `wezterm cli spawn --workspace`.
    pub name: String,
    /// Human-readable label.
    pub label: String,
}

fn default_shell() -> String {
    "zsh".to_string()
}

impl Default for ZllgConfig {
    fn default() -> Self {
        Self {
            default_shell: default_shell(),
            panes: vec![
                PaneConfig {
                    name: "editor".into(),
                    command: "hx".into(),
                    args: vec![".".into()],
                    index: 0,
                },
                PaneConfig {
                    name: "files".into(),
                    command: "yazi".into(),
                    args: vec![],
                    index: 1,
                },
                PaneConfig {
                    name: "git".into(),
                    command: "lazygit".into(),
                    args: vec![],
                    index: 2,
                },
                PaneConfig {
                    name: "shell".into(),
                    command: "nu".into(),
                    args: vec![],
                    index: 3,
                },
            ],
            workspaces: vec![
                WorkspaceConfig {
                    name: "monitor-1".into(),
                    label: "Main".into(),
                },
                WorkspaceConfig {
                    name: "monitor-2".into(),
                    label: "Secondary".into(),
                },
                WorkspaceConfig {
                    name: "monitor-3".into(),
                    label: "Tertiary".into(),
                },
            ],
        }
    }
}

/// Resolve the config file path.
pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("zllg")
        .join("config.toml")
}

/// Load config from disk, returning defaults if the file is absent.
pub fn load_config() -> Result<ZllgConfig> {
    let path = config_path();
    if !path.exists() {
        return Ok(ZllgConfig::default());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| ZllgError::config(e.to_string()))?;
    let cfg: ZllgConfig = toml::from_str(&raw)?;
    Ok(cfg)
}

/// Write the default config to disk (used by `zllg init`).
pub fn write_default_config() -> Result<PathBuf> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ZllgError::config(e.to_string()))?;
    }
    let default = ZllgConfig::default();
    let rendered = toml::to_string_pretty(&default)?;
    std::fs::write(&path, rendered).map_err(|e| ZllgError::config(e.to_string()))?;
    Ok(path)
}
