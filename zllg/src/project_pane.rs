use serde::{Deserialize, Serialize};

/// Project-aware pane configuration overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPaneConfig {
    /// Project type this config applies to.
    pub project_type: String,
    /// Pane-specific overrides.
    #[serde(default)]
    pub panes: Vec<PaneOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneOverride {
    /// Pane name to override.
    pub name: String,
    /// Override command.
    #[serde(default)]
    pub command: Option<String>,
    /// Override args.
    #[serde(default)]
    pub args: Option<Vec<String>>,
    /// Override size.
    #[serde(default)]
    pub size: Option<String>,
    /// Override split direction.
    #[serde(default)]
    pub split_direction: Option<String>,
}

impl Default for ProjectPaneConfig {
    fn default() -> Self {
        Self {
            project_type: "rust".into(),
            panes: vec![
                PaneOverride {
                    name: "watch".into(),
                    command: Some("cargo watch".into()),
                    args: Some(vec!["-x".into(), "check --message-format short".into()]),
                    size: None,
                    split_direction: None,
                },
                PaneOverride {
                    name: "editor".into(),
                    command: Some("helix".into()),
                    args: Some(vec![".".into()]),
                    size: None,
                    split_direction: None,
                },
            ],
        }
    }
}

/// Apply project-aware overrides to a base pane config.
pub fn apply_overrides(
    base_cmd: &str,
    base_args: &[String],
    project_cfg: &ProjectPaneConfig,
) -> (String, Vec<String>) {
    let found = project_cfg
        .panes
        .iter()
        .find(|p| p.name == "watch")
        .or_else(|| project_cfg.panes.iter().find(|p| p.name == "editor"));

    match found {
        Some(p) => {
            let cmd = p.command.as_deref().unwrap_or(base_cmd);
            let args = p.args.as_deref().unwrap_or(base_args);
            (cmd.to_string(), args.to_vec())
        }
        None => (base_cmd.to_string(), base_args.to_vec()),
    }
}
