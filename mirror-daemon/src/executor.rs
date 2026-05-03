use crate::reflection::*;
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

/// Executor for running pipelines (Nu scripts) in isolation
pub struct PipelineExecutor {
    /// Working directory for executions
    work_dir: PathBuf,
}

impl PipelineExecutor {
    pub fn new(work_dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let work_dir = work_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&work_dir)?;

        Ok(Self { work_dir })
    }

    /// Expose the working directory so callers can reconstruct an executor in
    /// a `spawn_blocking` closure (where `&self` cannot be captured).
    pub fn work_dir(&self) -> &Path {
        &self.work_dir
    }

    /// Execute a pipeline and produce a reflection envelope
    pub fn execute(
        &self,
        pipeline_path: impl AsRef<Path>,
        witness: String,
    ) -> std::io::Result<ReflectionEnvelope> {
        let pipeline_path = pipeline_path.as_ref();

        // Validate the file has a .nu extension (nushell script only)
        if pipeline_path.extension().and_then(|e| e.to_str()) != Some("nu") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!(
                    "Pipeline file must have .nu extension: {}",
                    pipeline_path.display()
                ),
            ));
        }

        let start_time = Instant::now();
        let timestamp = Utc::now();

        // Hash the pipeline content
        let pipeline_content = std::fs::read(pipeline_path)?;
        let content_hash = hash_data(&pipeline_content);

        // Create execution directory
        let exec_id = format!("exec_{}", timestamp.timestamp_millis());
        let exec_dir = self.work_dir.join(&exec_id);
        std::fs::create_dir_all(&exec_dir)?;

        // Execute the pipeline
        // For now, we assume Nu is available. Later, this could be configurable.
        // Pass the raw event payload as an environment variable so pipeline scripts
        // can access it via $env.MIRROR_PAYLOAD.
        let output = Command::new("nu")
            .arg(pipeline_path)
            .env("MIRROR_PAYLOAD", &witness)
            .current_dir(&exec_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        let duration = start_time.elapsed();

        // Capture outputs
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        // Exit code may be None if process was terminated by signal
        let exit_code = output.status.code().unwrap_or(-1);

        // Store full stdout/stderr as artifacts if non-empty
        let mut stdout_hash = None;
        let mut stderr_hash = None;

        if !stdout.is_empty() {
            let stdout_path = exec_dir.join("stdout.txt");
            std::fs::write(&stdout_path, &stdout)?;
            stdout_hash = Some(hash_file(&stdout_path)?);
        }

        if !stderr.is_empty() {
            let stderr_path = exec_dir.join("stderr.txt");
            std::fs::write(&stderr_path, &stderr)?;
            stderr_hash = Some(hash_file(&stderr_path)?);
        }

        // Truncate for envelope (human-readable summary only)
        let stdout = truncate_string(stdout, 10_000);
        let stderr = truncate_string(stderr, 10_000);

        // Scan execution directory for outputs
        let outputs = self.collect_outputs(&exec_dir)?;

        // Build the envelope
        let envelope = ReflectionEnvelope {
            id: String::new(), // Will be generated
            timestamp,
            transform: TransformWitness {
                content_hash,
                source_path: pipeline_path.to_path_buf(),
                version: None, // Could integrate with git later
            },
            inputs: vec![], // TODO: capture input fingerprints
            outputs,
            execution: ExecutionMeta {
                exit_code,
                stdout,
                stderr,
                stdout_hash,
                stderr_hash,
                duration_ms: duration.as_millis() as u64,
                witness,
            },
        };

        Ok(envelope)
    }

    /// Collect output artifacts from the execution directory
    fn collect_outputs(&self, exec_dir: &Path) -> std::io::Result<Vec<OutputArtifact>> {
        let mut outputs = Vec::new();

        if !exec_dir.exists() {
            return Ok(outputs);
        }

        // Walk the directory tree
        for entry in std::fs::read_dir(exec_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let hash = hash_file(&path)?;
                let artifact_type = infer_artifact_type(&path);

                outputs.push(OutputArtifact {
                    path,
                    hash,
                    artifact_type,
                });
            }
        }

        Ok(outputs)
    }
}

/// Truncate a string to a maximum length, adding ellipsis if truncated
fn truncate_string(s: String, max_len: usize) -> String {
    if s.len() <= max_len {
        s
    } else {
        format!(
            "{}... [truncated {} bytes]",
            &s[..max_len],
            s.len() - max_len
        )
    }
}

/// Infer artifact type from file extension
///
/// Note: This is intentionally simple. The artifact type is a *hint* for humans,
/// not authoritative classification. The actual content is in content-addressed
/// storage and can be verified via hash. We don't need MIME detection because
/// we're not serving files - we're witnessing executions.
fn infer_artifact_type(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_string() {
        let short = "hello".to_string();
        assert_eq!(truncate_string(short, 10), "hello");

        let long = "a".repeat(1000);
        let truncated = truncate_string(long, 100);
        assert!(truncated.len() < 200); // 100 + ellipsis
        assert!(truncated.contains("truncated"));
    }

    #[test]
    fn test_infer_artifact_type() {
        assert_eq!(infer_artifact_type(Path::new("test.json")), "json");
        assert_eq!(infer_artifact_type(Path::new("report.xlsx")), "xlsx");
        assert_eq!(infer_artifact_type(Path::new("no_extension")), "unknown");
    }
}
