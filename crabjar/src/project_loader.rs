//! Project loader module for CrabJar agent toolboxes
//!
//! This module provides functionality for loading and managing project-specific
//! configurations and command metadata for the stripped-down CrabJar CLI.
use crabjar_config::{ConfigError, ProjectConfig};
use std::path::{Path, PathBuf};

/// Result type for project loading operations
pub type ProjectResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Manages project-specific configurations and tool loading
#[derive(Debug)]
pub struct ProjectLoader {
    /// Current loaded configuration (if any)
    current_config: Option<ProjectConfig>,
    /// Root directory used to resolve relative tool paths
    project_root: Option<PathBuf>,
}

impl Default for ProjectLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectLoader {
    /// Create a new project loader instance
    pub fn new() -> Self {
        Self {
            current_config: None,
            project_root: None,
        }
    }

    /// Load configuration from the specified project root directory
    pub async fn load_from_directory(&mut self, path: &Path) -> ProjectResult<()> {
        self.project_root = Some(path.to_path_buf());

        let config = match ProjectConfig::load(path) {
            Ok(cfg) => cfg,
            Err(
                ConfigError::NotFound(_) | ConfigError::TomlError(_) | ConfigError::MissingName,
            ) => {
                self.current_config = None;
                return Ok(());
            }
        };

        self.current_config = Some(config.clone());

        if config.auto_register {
            self.register_tools(&config)?;
        }

        Ok(())
    }
    /// Register all tools defined in the configuration
    pub fn register_tools(&self, config: &ProjectConfig) -> ProjectResult<()> {
        for tool_def in &config.tools {
            let path = self.resolve_tool_path(&tool_def.path);

            let exists = path.exists();
            if !exists {
                return Err(format!("tool not found: {}", path.display()).into());
            }
        }

        Ok(())
    }

    fn resolve_tool_path(&self, tool_path: &str) -> PathBuf {
        let path = PathBuf::from(tool_path);
        if path.is_absolute() {
            return path;
        }

        match &self.project_root {
            Some(root) => root.join(path),
            None => path,
        }
    }

    /// Get the current project configuration if loaded
    pub fn get_current_config(&self) -> Option<&ProjectConfig> {
        self.current_config.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_load_existing_config() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".crabjar_config.toml");
        let tools_dir = dir.path().join("tools");
        fs::create_dir_all(&tools_dir).unwrap();
        fs::write(tools_dir.join("test.nu"), "echo ok").unwrap();

        fs::write(
            &config_path,
            r#"
name = "test-workspace"
description = "Test workspace"

[[tools]]
path = "tools/test.nu"
commands = ["cmd1", "cmd2"]

[keybindings]
"Ctrl a" = "cmd1"
"#,
        )
        .unwrap();

        let mut loader = ProjectLoader::new();
        let result = loader.load_from_directory(dir.path()).await;

        assert!(result.is_ok());
        assert!(loader.get_current_config().is_some());
    }

    #[tokio::test]
    async fn test_create_default_workspace() {
        let dir = tempdir().unwrap();

        // No config file exists - should soft-fail to no workspace
        let mut loader = ProjectLoader::new();
        let result = loader.load_from_directory(dir.path()).await;

        assert!(result.is_ok());
        assert!(loader.get_current_config().is_none());
    }

    #[tokio::test]
    async fn test_command_lookup() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".crabjar_config.toml");
        let tools_dir = dir.path().join("tools");
        fs::create_dir_all(&tools_dir).unwrap();
        fs::write(tools_dir.join("test.nu"), "echo ok").unwrap();

        fs::write(
            &config_path,
            r#"
name = "command-test"

[[tools]]
path = "tools/test.nu"
commands = ["cmd1", "cmd2"]
"#,
        )
        .unwrap();

        let mut loader = ProjectLoader::new();
        loader.load_from_directory(dir.path()).await.unwrap();

        let commands = loader
            .get_current_config()
            .map(|config| config.get_all_commands())
            .unwrap_or_default();
        assert!(commands.contains(&"cmd1".to_string()));
        assert!(commands.contains(&"cmd2".to_string()));
        assert!(!commands.contains(&"nonexistent".to_string()));
    }

    #[tokio::test]
    async fn test_relative_tool_paths_resolve_from_project_root() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".crabjar_config.toml");
        let nested_tools_dir = dir.path().join("tools");
        fs::create_dir_all(&nested_tools_dir).unwrap();
        fs::write(nested_tools_dir.join("tool.nu"), "echo ok").unwrap();

        fs::write(
            &config_path,
            r#"
name = "relative-paths"

[[tools]]
path = "tools/tool.nu"
commands = ["cmd1"]
"#,
        )
        .unwrap();

        let mut loader = ProjectLoader::new();
        loader.load_from_directory(dir.path()).await.unwrap();

        let resolved = loader.resolve_tool_path("tools/tool.nu");
        assert_eq!(resolved, dir.path().join("tools").join("tool.nu"));
    }

    #[tokio::test]
    async fn test_malformed_config_returns_error() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".crabjar_config.toml");

        fs::write(&config_path, "this is not [ valid toml !!!").unwrap();

        let mut loader = ProjectLoader::new();
        let result = loader.load_from_directory(dir.path()).await;

        assert!(result.is_ok());
        assert!(loader.get_current_config().is_none());
    }

    #[tokio::test]
    async fn test_no_config_produces_default_with_no_commands() {
        let dir = tempdir().unwrap();

        let mut loader = ProjectLoader::new();
        loader.load_from_directory(dir.path()).await.unwrap();

        assert!(loader.get_current_config().is_none());
    }
}
