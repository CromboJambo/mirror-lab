use serde::{Deserialize, Serialize};

/// The IDE dashboard state — what each pane is doing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardState {
    /// Current project type.
    pub project_type: String,
    /// Pane statuses keyed by name.
    #[serde(default)]
    pub panes: Vec<PaneStatus>,
    /// Time since last build (for Rust projects).
    #[serde(default)]
    pub last_build: Option<String>,
    /// Git branch status.
    #[serde(default)]
    pub git_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneStatus {
    /// Pane name.
    pub name: String,
    /// Whether the pane is visible.
    pub visible: bool,
    /// Whether the pane is embedded (inside Zellij) or ejected.
    pub embedded: bool,
    /// Pane index in the Zellij layout.
    pub index: usize,
}

/// Build a dashboard state from config.
pub fn build_dashboard(
    cfg: &crate::config::ZllgConfig,
    pt: crate::detect::ProjectType,
) -> DashboardState {
    DashboardState {
        project_type: pt.to_string(),
        panes: cfg
            .panes
            .iter()
            .map(|p| PaneStatus {
                name: p.name.clone(),
                visible: true,
                embedded: true,
                index: p.index,
            })
            .collect(),
        last_build: None,
        git_branch: None,
    }
}

/// Render the dashboard as a simple text block (for status-bar plugin).
pub fn render_dashboard(state: &DashboardState) -> String {
    let mut lines = Vec::new();
    lines.push(format!("zllg IDE — {}", state.project_type));
    for pane in &state.panes {
        let mark = if pane.visible { "◉" } else { "○" };
        let embed = if pane.embedded { "in" } else { "out" };
        lines.push(format!("  {mark} {0:<12} {embed}", pane.name));
    }
    lines.join("\n")
}
