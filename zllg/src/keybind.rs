use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A keybind entry for a Zellij pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keybind {
    /// The modifier combo (e.g. "alt", "ctrl-shift").
    pub modifier: String,
    /// The key (e.g. "f", "g", "w").
    pub key: String,
    /// The action to perform (e.g. "toggle-pane-frames", "focus-pane-with-index").
    pub action: String,
    /// Optional target pane name for the action.
    #[serde(default)]
    pub target: Option<String>,
}

/// Keybind config for Zellij (`~/.config/zllg/keybinds.toml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindConfig {
    #[serde(default)]
    pub keybinds: Vec<Keybind>,
}

impl Default for KeybindConfig {
    fn default() -> Self {
        Self {
            keybinds: vec![
                Keybind {
                    modifier: "alt".into(),
                    key: "f".into(),
                    action: "toggle-pane-frames".into(),
                    target: Some("files".into()),
                },
                Keybind {
                    modifier: "alt".into(),
                    key: "g".into(),
                    action: "toggle-pane-frames".into(),
                    target: Some("git".into()),
                },
                Keybind {
                    modifier: "alt".into(),
                    key: "e".into(),
                    action: "toggle-pane-frames".into(),
                    target: Some("editor".into()),
                },
                Keybind {
                    modifier: "alt".into(),
                    key: "s".into(),
                    action: "toggle-pane-frames".into(),
                    target: Some("shell".into()),
                },
                Keybind {
                    modifier: "ctrl".into(),
                    key: "w".into(),
                    action: "focus-pane-with-index".into(),
                    target: None,
                },
                Keybind {
                    modifier: "alt".into(),
                    key: "p".into(),
                    action: "toggle-pane-embed-or-eject".into(),
                    target: None,
                },
            ],
        }
    }
}

/// Resolve the keybind config file path.
pub fn keybind_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("zllg")
        .join("keybinds.toml")
}

/// Load keybind config from disk, returning defaults if absent.
pub fn load_keybinds() -> anyhow::Result<KeybindConfig> {
    let path = keybind_path();
    if !path.exists() {
        return Ok(KeybindConfig::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    let cfg: KeybindConfig = toml::from_str(&raw)?;
    Ok(cfg)
}

/// Write default keybinds to disk.
pub fn write_default_keybinds() -> anyhow::Result<PathBuf> {
    let path = keybind_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let default = KeybindConfig::default();
    let rendered = toml::to_string_pretty(&default)?;
    std::fs::write(&path, rendered)?;
    Ok(path)
}

/// Render keybinds as Zellij KDL plugin config for embedding in layouts.
pub fn render_keybind_kdl() -> String {
    let cfg = KeybindConfig::default();
    let mut lines = Vec::new();
    lines.push("plugin location=\"zellij:plugin-kdl\"".into());
    for kb in &cfg.keybinds {
        let target = kb
            .target
            .as_deref()
            .map(|t| format!(" target=\"{t}\""))
            .unwrap_or_default();
        lines.push(format!(
            "    keybind modifier=\"{}\" key=\"{}\" action=\"{}\"{}",
            kb.modifier, kb.key, kb.action, target
        ));
    }
    lines.join("\n")
}

/// Render keybinds as a Zellij KDL file (standalone keybind config).
pub fn render_keybind_file() -> String {
    let cfg = KeybindConfig::default();
    let mut lines = Vec::new();
    lines.push("keybinds {".into());
    for kb in &cfg.keybinds {
        let target = kb
            .target
            .as_deref()
            .map(|t| format!(" target=\"{t}\""))
            .unwrap_or_default();
        lines.push(format!(
            "    keybind modifier=\"{}\" key=\"{}\" action=\"{}\"{}",
            kb.modifier, kb.key, kb.action, target
        ));
    }
    lines.push("}".into());
    lines.join("\n")
}
