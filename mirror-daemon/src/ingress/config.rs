// src/config.rs
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub capture: CaptureConfig,
    pub processing: ProcessingConfig,
    pub storage: StorageConfig,
    pub retention: RetentionConfig,
    #[allow(dead_code)]
    pub ocr: OcrConfig,
    pub transcription: TranscriptionConfig,
}

#[derive(Debug, Deserialize)]
pub struct CaptureConfig {
    pub watch_dir: PathBuf,
    pub extensions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProcessingConfig {
    pub staging_dir: PathBuf,
    pub margin: String,
    pub chunk_max_seconds: u64,
}

#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    pub db_path: PathBuf,
    pub chunks_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct RetentionConfig {
    pub fine_grain_days: u64,
    pub coarse_grain_days: u64,
}

#[derive(Debug, Deserialize)]
pub struct OcrConfig {
    #[allow(dead_code)]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct TranscriptionConfig {
    pub enabled: bool,
    pub model_path: Option<PathBuf>,
    #[cfg_attr(not(feature = "transcription"), allow(dead_code))]
    pub language: Option<String>,
    #[cfg_attr(not(feature = "transcription"), allow(dead_code))]
    pub threads: Option<u8>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("Could not read config at {}", path.display()))?;
        let config: Config =
            toml::from_str(&raw).with_context(|| "Failed to parse ingress.toml")?;
        Ok(config)
    }
}
