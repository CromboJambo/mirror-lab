use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A workspace definition for WezTerm monitor mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDef {
    /// WezTerm workspace name.
    pub name: String,
    /// Human-readable label.
    pub label: String,
    /// Default command for the workspace shell.
    #[serde(default)]
    pub default_cmd: String,
    /// Optional args.
    #[serde(default)]
    pub default_args: Vec<String>,
}

impl Default for WorkspaceDef {
    fn default() -> Self {
        Self {
            name: "monitor-1".into(),
            label: "Main".into(),
            default_cmd: "nu".into(),
            default_args: vec![],
        }
    }
}

/// Workspace config (`~/.config/zllg/workspaces.toml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub workspaces: Vec<WorkspaceDef>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            workspaces: vec![
                WorkspaceDef {
                    name: "monitor-1".into(),
                    label: "Main".into(),
                    default_cmd: "nu".into(),
                    default_args: vec![],
                },
                WorkspaceDef {
                    name: "monitor-2".into(),
                    label: "Secondary".into(),
                    default_cmd: "nu".into(),
                    default_args: vec![],
                },
                WorkspaceDef {
                    name: "monitor-3".into(),
                    label: "Tertiary".into(),
                    default_cmd: "nu".into(),
                    default_args: vec![],
                },
            ],
        }
    }
}

/// Resolve the workspace config file path.
pub fn workspace_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("zllg")
        .join("workspaces.toml")
}

/// Load workspace config from disk, returning defaults if absent.
pub fn load_workspaces() -> anyhow::Result<WorkspaceConfig> {
    let path = workspace_path();
    if !path.exists() {
        return Ok(WorkspaceConfig::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    let cfg: WorkspaceConfig = toml::from_str(&raw)?;
    Ok(cfg)
}

/// Write default workspaces to disk.
pub fn write_default_workspaces() -> anyhow::Result<PathBuf> {
    let path = workspace_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let default = WorkspaceConfig::default();
    let rendered = toml::to_string_pretty(&default)?;
    std::fs::write(&path, rendered)?;
    Ok(path)
}

/// Find a workspace by name.
pub fn find_workspace<'a>(workspaces: &'a [WorkspaceDef], name: &str) -> Option<&'a WorkspaceDef> {
    workspaces.iter().find(|w| w.name == name)
}
