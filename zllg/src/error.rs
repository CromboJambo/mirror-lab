use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZllgError {
    #[error("required tool not found: {0}")]
    MissingTool(String),

    #[error("zellij subprocess failed: {0}")]
    ZellijError(String),

    #[error("wezterm subprocess failed: {0}")]
    WeztermError(String),

    #[error("config error: {0}")]
    ConfigError(String),

    #[error("layout error: {0}")]
    LayoutError(String),

    #[error("mirror-log append failed: {0}")]
    LoggingError(String),

    #[error("execution interrupted by gate")]
    Interrupted,

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("sql error: {0}")]
    Sql(#[from] rusqlite::Error),

    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("toml serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}

impl ZllgError {
    pub fn missing_tool(name: &str) -> Self {
        Self::MissingTool(name.to_string())
    }

    pub fn zellij(msg: &str) -> Self {
        Self::ZellijError(msg.to_string())
    }

    pub fn wezterm(msg: &str) -> Self {
        Self::WeztermError(msg.to_string())
    }

    pub fn config(msg: &str) -> Self {
        Self::ConfigError(msg.to_string())
    }

    pub fn layout(msg: impl Into<String>) -> Self {
        Self::LayoutError(msg.into())
    }

    pub fn logging(msg: impl Into<String>) -> Self {
        Self::LoggingError(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, ZllgError>;
