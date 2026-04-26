use std::path::Path;

/// The project type detected from the working directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectType {
    Rust,
    Node,
    Python,
    Nix,
    Default,
}

impl ProjectType {
    /// Returns the layout file stem (e.g. "rust" → `rust.kdl`).
    pub fn layout_name(&self) -> &'static str {
        match self {
            ProjectType::Rust => "rust",
            ProjectType::Node => "node",
            ProjectType::Python => "python",
            ProjectType::Nix => "nix",
            ProjectType::Default => "default",
        }
    }
}

impl std::fmt::Display for ProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.layout_name())
    }
}

impl std::str::FromStr for ProjectType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "rust" => ProjectType::Rust,
            "node" => ProjectType::Node,
            "python" => ProjectType::Python,
            "nix" => ProjectType::Nix,
            _ => ProjectType::Default,
        })
    }
}

/// Detect the project type from the given directory.
pub fn detect_project_type(dir: &Path) -> ProjectType {
    let markers: &[(&str, ProjectType)] = &[
        ("Cargo.toml", ProjectType::Rust),
        ("package.json", ProjectType::Node),
        ("pyproject.toml", ProjectType::Python),
        ("setup.py", ProjectType::Python),
        ("flake.nix", ProjectType::Nix),
    ];

    for (marker, project_type) in markers {
        if dir.join(marker).exists() {
            return project_type.clone();
        }
    }

    ProjectType::Default
}
