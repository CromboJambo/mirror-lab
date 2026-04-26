use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Project-aware layout config (`~/.config/zllg/project_layouts.toml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLayoutConfig {
    #[serde(default)]
    pub layouts: Vec<ProjectLayoutDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLayoutDef {
    /// Project type this layout applies to.
    pub project_type: String,
    /// KDL layout content.
    pub kdl: String,
}

impl Default for ProjectLayoutConfig {
    fn default() -> Self {
        Self {
            layouts: vec![
                ProjectLayoutDef {
                    project_type: "rust".into(),
                    kdl: crate::layout::RUST_LAYOUT_KDL.to_string(),
                },
                ProjectLayoutDef {
                    project_type: "node".into(),
                    kdl: crate::layout::NODE_LAYOUT_KDL.to_string(),
                },
                ProjectLayoutDef {
                    project_type: "python".into(),
                    kdl: crate::layout::PYTHON_LAYOUT_KDL.to_string(),
                },
                ProjectLayoutDef {
                    project_type: "nix".into(),
                    kdl: crate::layout::NIX_LAYOUT_KDL.to_string(),
                },
                ProjectLayoutDef {
                    project_type: "default".into(),
                    kdl: crate::layout::DEFAULT_LAYOUT_KDL.to_string(),
                },
            ],
        }
    }
}

/// Resolve the project layout config file path.
pub fn project_layout_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("zllg")
        .join("project_layouts.toml")
}

/// Load project layout config from disk, returning defaults if absent.
pub fn load_project_layouts() -> anyhow::Result<ProjectLayoutConfig> {
    let path = project_layout_path();
    if !path.exists() {
        return Ok(ProjectLayoutConfig::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    let cfg: ProjectLayoutConfig = toml::from_str(&raw)?;
    Ok(cfg)
}

/// Write default project layouts to disk.
pub fn write_default_project_layouts() -> anyhow::Result<PathBuf> {
    let path = project_layout_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let default = ProjectLayoutConfig::default();
    let rendered = toml::to_string_pretty(&default)?;
    std::fs::write(&path, rendered)?;
    Ok(path)
}

/// Find a project layout by type.
pub fn find_project_layout<'a>(
    layouts: &'a [ProjectLayoutDef],
    project_type: &str,
) -> Option<&'a ProjectLayoutDef> {
    layouts.iter().find(|l| l.project_type == project_type)
}
