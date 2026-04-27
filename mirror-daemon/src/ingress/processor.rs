use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::{Map, Value, json};
use std::{path::Path, path::PathBuf, process::Command};
use tracing::{info, warn};

use crate::ingress::config::ProcessingConfig;
use crate::ingress::db::Chunk;
use crate::ingress::sanitizer::Sanitizer;

pub struct ProcessedRecording {
    pub chunks: Vec<Chunk>,
    pub source_file: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct CaptureMetadata {
    window_title: Option<String>,
    metadata_json: Option<String>,
}

/// Entry point: process one OBS recording file end to end.
pub async fn process_recording(
    input: &Path,
    config: &ProcessingConfig,
    chunks_dir: &Path,
) -> Result<ProcessedRecording> {
    let sanitizer = Sanitizer::new()?;
    let source_file = sanitizer.sanitize_text(
        input
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .as_ref(),
    );

    if source_file.len() > 255 {
        warn!(
            "Source file name '{}' is extremely long; truncation may occur in downstream systems.",
            source_file
        );
    }

    info!("Processing: {}", source_file);

    // 1. Get raw duration before editing
    let raw_duration = probe_duration(input.to_path_buf())?;
    info!("Raw duration: {:.1}s", raw_duration);

    // 2. Run auto-editor
    let edited = run_auto_editor(input, config).await?;
    let edited_duration = probe_duration(edited.clone())?;
    let removed = raw_duration - edited_duration;

    info!(
        "After auto-editor: {:.1}s ({:.1}s removed, {:.0}%)",
        edited_duration,
        removed,
        (removed / raw_duration) * 100.0
    );

    // 3. Split into chunks
    let chunk_paths =
        split_into_chunks(&edited, chunks_dir, &source_file, config.chunk_max_seconds)?;
    info!("Split into {} chunks", chunk_paths.len());

    // 4. Build chunk records
    let recording_start = file_start_time(input);
    let ratio: f64 = edited_duration / raw_duration;
    let capture_metadata = probe_capture_metadata(input, &sanitizer)?;

    let chunks = chunk_paths
        .into_iter()
        .enumerate()
        .map(|(i, path)| {
            let duration = probe_duration(path.clone()).unwrap_or(0.0);
            let chunk_offset_raw = (i as f64 * config.chunk_max_seconds as f64) / ratio;
            let started_at = recording_start
                + chrono::Duration::milliseconds((chunk_offset_raw * 1000.0) as i64);

            Chunk {
                id: None,
                source_file: source_file.clone(),
                chunk_index: i as u32,
                chunk_path: path.to_string_lossy().to_string(),
                started_at,
                duration_secs: duration,
                raw_duration_secs: duration / ratio,
                ocr_text: Some(sanitizer.sanitize_text("")), // Placeholder for future OCR integration
                transcript: Some(sanitizer.sanitize_text("")), // Placeholder for future transcription integration
                window_title: capture_metadata.window_title.clone(),
                metadata: capture_metadata.metadata_json.clone(),
                source_type: "filesystem".to_string(),
                importance_score: 1.0,
                distillation_tier: 0,
                retained: false,
                created_at: Utc::now(),
            }
        })
        .collect();

    Ok(ProcessedRecording {
        chunks,
        source_file,
    })
}

fn probe_capture_metadata(input: &Path, sanitizer: &Sanitizer) -> Result<CaptureMetadata> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format_tags:stream_tags",
            "-of",
            "json",
        ])
        .arg(input)
        .output()
        .context("ffprobe not found — install ffmpeg")?;

    if !output.status.success() {
        warn!("ffprobe metadata probe failed for {}", input.display());
        return Ok(CaptureMetadata::default());
    }

    let parsed: Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse ffprobe metadata JSON")?;
    Ok(extract_capture_metadata(&parsed, sanitizer))
}

fn extract_capture_metadata(parsed: &Value, sanitizer: &Sanitizer) -> CaptureMetadata {
    let mut promoted = Map::new();
    let mut candidates = Vec::new();

    for key in ["window_title", "title", "handler_name", "comment"] {
        if let Some(value) = find_tag_value(parsed, key) {
            let cleaned = sanitizer.sanitize_text(value);
            if !cleaned.is_empty() {
                if key == "window_title" || key == "title" {
                    candidates.push(cleaned.clone());
                }
                promoted.insert(key.to_string(), Value::String(cleaned));
            }
        }
    }

    let window_title = candidates.into_iter().next();

    if let Some(window_title) = &window_title {
        promoted.insert(
            "window_title_promoted".to_string(),
            Value::String(window_title.clone()),
        );
    }

    let metadata_json = if promoted.is_empty() {
        None
    } else {
        Some(json!({ "capture_metadata": promoted }).to_string())
    };

    CaptureMetadata {
        window_title,
        metadata_json,
    }
}

fn find_tag_value<'a>(parsed: &'a Value, target_key: &str) -> Option<&'a str> {
    let target_key = target_key.to_ascii_lowercase();

    parsed
        .get("format")
        .and_then(|format| format.get("tags"))
        .into_iter()
        .chain(
            parsed
                .get("streams")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|stream| stream.get("tags")),
        )
        .filter_map(Value::as_object)
        .find_map(|tags| {
            tags.iter().find_map(|(key, value)| {
                (key.to_ascii_lowercase() == target_key)
                    .then(|| value.as_str())
                    .flatten()
            })
        })
}

/// Run auto-editor on the input file, return path to output file.
pub async fn run_auto_editor(input: &Path, config: &ProcessingConfig) -> Result<PathBuf> {
    use std::io::ErrorKind;

    std::fs::create_dir_all(&config.staging_dir)?;

    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let ext = input.extension().unwrap_or_default().to_string_lossy();
    let output = config.staging_dir.join(format!("{}_edited.{}", stem, ext));

    let status = Command::new("auto-editor")
        .arg(input)
        .arg("--output")
        .arg(&output)
        .arg("--margin")
        .arg(&config.margin)
        .arg("--no-open")
        .status()
        .map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                anyhow::anyhow!(
                    "auto-editor is not installed or not in PATH. Install `auto-editor` with your package manager, then restart ingress."
                )
            } else {
                anyhow::anyhow!(e).context("Failed to run auto-editor")
            }
        })?;

    if !status.success() {
        anyhow::bail!("auto-editor exited with status: {}", status);
    }

    Ok(output)
}

/// Check for audio stream using ffprobe.
#[allow(dead_code)]
fn has_audio_stream(path: PathBuf) -> Result<bool> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=index",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(&path)
        .output()
        .context("ffprobe not found — install ffmpeg")?;

    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

/// Use ffprobe to get duration in seconds.
fn probe_duration(path: PathBuf) -> Result<f64> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(&path)
        .output()
        .context("ffprobe not found — install ffmpeg")?;

    let s = String::from_utf8_lossy(&output.stdout);
    s.trim()
        .parse::<f64>()
        .context("Could not parse duration from ffprobe")
}

/// Split a video into max_seconds chunks using ffmpeg segment muxer.
fn split_into_chunks(
    input: &Path,
    chunks_dir: &Path,
    source_stem: &str,
    max_seconds: u64,
) -> Result<Vec<PathBuf>> {
    let safe_stem = source_stem.replace([' ', ':'], "_");
    let out_dir = chunks_dir.join(&safe_stem);
    std::fs::create_dir_all(&out_dir)?;

    let pattern = out_dir.join("chunk_%04d.mkv");

    let status = Command::new("ffmpeg")
        .args(["-i"])
        .arg(input)
        .args([
            "-c",
            "copy",
            "-f",
            "segment",
            "-segment_time",
            &max_seconds.to_string(),
            "-reset_timestamps",
            "1",
            "-avoid_negative_ts",
            "make_zero",
        ])
        .arg(&pattern)
        .arg("-y")
        .status()?;

    if !status.success() {
        anyhow::bail!("ffmpeg segment split failed");
    }

    let mut paths: Vec<PathBuf> = std::fs::read_dir(&out_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "mkv").unwrap_or(false))
        .collect();

    paths.sort();

    if paths.is_empty() {
        warn!("No chunks produced from {}", input.display());
    }

    Ok(paths)
}

/// Get wall clock start time from file metadata (mtime as fallback).
fn file_start_time(path: &Path) -> chrono::DateTime<Utc> {
    if let Ok(meta) = std::fs::metadata(path)
        && let Ok(modified) = meta.modified()
    {
        let duration = modified
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        return chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
            .unwrap_or(Utc::now());
    }
    Utc::now()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_capture_metadata_promotes_window_title_and_serializes_metadata() {
        let sanitizer = Sanitizer::new().expect("sanitizer");
        let parsed = json!({
            "format": {
                "tags": {
                    "TITLE": "Neovim - mirror-lab"
                }
            },
            "streams": [
                {
                    "tags": {
                        "handler_name": "Screen Capture"
                    }
                }
            ]
        });

        let metadata = extract_capture_metadata(&parsed, &sanitizer);
        assert_eq!(
            metadata.window_title.as_deref(),
            Some("Neovim - mirror-lab")
        );

        let metadata_json = metadata.metadata_json.expect("metadata json");
        let value: Value = serde_json::from_str(&metadata_json).expect("parse metadata json");
        assert_eq!(
            value["capture_metadata"]["window_title_promoted"].as_str(),
            Some("Neovim - mirror-lab")
        );
        assert_eq!(
            value["capture_metadata"]["handler_name"].as_str(),
            Some("Screen Capture")
        );
    }

    #[test]
    fn extract_capture_metadata_ignores_missing_tags() {
        let sanitizer = Sanitizer::new().expect("sanitizer");
        let parsed = json!({ "format": {}, "streams": [] });

        let metadata = extract_capture_metadata(&parsed, &sanitizer);
        assert_eq!(metadata, CaptureMetadata::default());
    }
}
