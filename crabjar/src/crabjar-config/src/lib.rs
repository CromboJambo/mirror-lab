//! crabjar-config
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration file not found: {0}")]
    NotFound(PathBuf),
    #[error("Failed to parse TOML: {0}")]
    TomlError(String),
    #[error("Workspace name not specified")]
    MissingName,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub path: String,
    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(rename = "name")]
    pub workspace_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub knowledge_store_path: Option<String>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    #[serde(default, rename = "keybindings")]
    pub keybindings: HashMap<String, String>,
    #[serde(default = "default_true", skip_serializing_if = "bool::clone")]
    pub auto_register: bool,
}

fn default_true() -> bool {
    true
}

impl ProjectConfig {
    pub fn load(project_root: &Path) -> Result<Self, ConfigError> {
        let config_path = project_root.join(".crabjar_config.toml");
        if !config_path.exists() {
            return Err(ConfigError::NotFound(config_path));
        }
        let content =
            fs::read_to_string(&config_path).map_err(|e| ConfigError::TomlError(e.to_string()))?;
        Self::parse_from_str(&content)
    }

    pub fn parse_from_str(toml_str: &str) -> Result<Self, ConfigError> {
        let config: ProjectConfig =
            toml::from_str(toml_str).map_err(|e| ConfigError::TomlError(e.to_string()))?;
        if config.workspace_name.is_empty() {
            return Err(ConfigError::MissingName);
        }
        Ok(config)
    }

    pub fn get_all_commands(&self) -> Vec<String> {
        self.tools.iter().flat_map(|t| t.commands.clone()).collect()
    }

    pub fn has_command(&self, command: &str) -> bool {
        self.tools
            .iter()
            .flat_map(|t| t.commands.iter())
            .any(|c| c == command)
    }

    pub fn get_keybinding_action(&self, key: &str) -> Option<String> {
        self.keybindings.get(key).cloned()
    }
}

#[derive(Debug, Default)]
pub struct ProjectConfigBuilder {
    name: String,
    description: Option<String>,
    knowledge_store_path: Option<String>,
    tools: Vec<ToolDefinition>,
    keybindings: HashMap<String, String>,
    auto_register: bool,
}

impl ProjectConfigBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            auto_register: true,
            ..Default::default()
        }
    }
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
    pub fn add_tool(mut self, path: impl Into<String>, cmds: Vec<String>) -> Self {
        self.tools.push(ToolDefinition {
            path: path.into(),
            commands: cmds,
        });
        self
    }
    pub fn knowledge_store_path(mut self, path: impl Into<String>) -> Self {
        self.knowledge_store_path = Some(path.into());
        self
    }
    pub fn keybinding(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.keybindings.insert(key.into(), value.into());
        self
    }
    pub fn no_auto_register(mut self) -> Self {
        self.auto_register = false;
        self
    }
    pub fn build(self) -> Result<ProjectConfig, ConfigError> {
        if self.name.is_empty() {
            return Err(ConfigError::MissingName);
        }
        Ok(ProjectConfig {
            workspace_name: self.name,
            description: self.description,
            knowledge_store_path: self.knowledge_store_path,
            tools: self.tools,
            keybindings: self.keybindings,
            auto_register: self.auto_register,
        })
    }
}

pub fn generate_template(name: &str) -> String {
    format!(
        "name = \"{}\"\ndescription = \"Custom CrabJar workspace for {}\"\n\nauto_register = true\n\n[[tools]]\npath = \"data-transformations.nu\"\ncommands = [\"load-data\", \"transform-pipeline\"]\n\n[keybindings]\n\"Ctrl a\" = \"load-data\"\n",
        name, name
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse() {
        let toml_str = r#"
name = "test"

[[tools]]
path = "t.nu"
commands = ["cmd"]
"#;
        assert!(ProjectConfig::parse_from_str(toml_str).is_ok());
    }
}
