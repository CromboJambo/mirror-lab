use serde::{Deserialize, Serialize};
use serde_json::json;

use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("config file not found")]
    ConfigNotFound,
    #[error("config file malformed: {0}")]
    ConfigMalformed(String),
    #[error("currency invalid: {0}")]
    CurrencyInvalid(String),
    #[error("export directory missing marker")]
    ExportGuard,
    #[error("alias not found: {0}")]
    AliasNotFound(String),
    #[error("io error")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBurnConfig {
    pub workspace: Option<String>,
    pub currency: String,
    pub plan: Option<String>,
    pub model_aliases: BTreeMap<String, String>,
}

impl Default for CodeBurnConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeBurnConfig {
    pub fn new() -> Self {
        Self {
            workspace: None,
            currency: "USD".to_string(),
            plan: None,
            model_aliases: BTreeMap::new(),
        }
    }

    pub fn load(project_root: &std::path::Path) -> Result<Self, Error> {
        let config_path = project_root.join(".codeburn_config.toml");

        if !config_path.exists() {
            return Ok(Self::new());
        }

        let content = std::fs::read_to_string(&config_path)?;

        toml::from_str(&content).map_err(|err| Error::ConfigMalformed(err.to_string()))
    }

    pub fn plan_usage(&self, plan_name: &str) -> Result<serde_json::Value, Error> {
        Ok(json!({
            "plan": plan_name,
            "usage": 0.0,
            "remaining": 0.0,
        }))
    }
}
