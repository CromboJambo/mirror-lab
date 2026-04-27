use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// A reflection envelope - immutable witness of a pipeline execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionEnvelope {
    /// Unique identifier (hash of execution)
    pub id: String,

    /// When this reflection was created
    pub timestamp: DateTime<Utc>,

    /// The pipeline that was executed
    pub transform: TransformWitness,

    /// Input fingerprints
    pub inputs: Vec<InputFingerprint>,

    /// Output artifacts
    pub outputs: Vec<OutputArtifact>,

    /// Execution metadata
    pub execution: ExecutionMeta,
}

/// Witness of the transform that was executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformWitness {
    /// Hash of the pipeline/script content
    pub content_hash: String,

    /// Path to the pipeline source
    pub source_path: PathBuf,

    /// Version/commit if tracked
    pub version: Option<String>,
}

/// Fingerprint of an input source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputFingerprint {
    /// Source identifier (file path, URL, etc)
    pub source: String,

    /// Hash of the input data
    pub hash: String,

    /// Timestamp when captured
    pub captured_at: DateTime<Utc>,

    /// Schema hint (if known)
    pub schema: Option<String>,
}

/// An output artifact produced by the pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputArtifact {
    /// Path where artifact is stored (content-addressed)
    pub path: PathBuf,

    /// Hash of the artifact
    pub hash: String,

    /// Type/format hint
    pub artifact_type: String,
}

/// Execution metadata - what actually happened
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMeta {
    /// Exit code of the pipeline
    pub exit_code: i32,

    /// Standard output (truncated if too long)
    pub stdout: String,

    /// Standard error (truncated if too long)
    pub stderr: String,

    /// Full stdout artifact hash (if non-empty)
    pub stdout_hash: Option<String>,

    /// Full stderr artifact hash (if non-empty)
    pub stderr_hash: Option<String>,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Who/what triggered this execution
    pub witness: String,
}

impl ReflectionEnvelope {
    /// Generate a unique ID for this reflection based on its content
    pub fn generate_id(&self) -> String {
        let mut hasher = Sha256::new();

        // Hash the essential components
        hasher.update(self.timestamp.to_rfc3339().as_bytes());
        hasher.update(self.transform.content_hash.as_bytes());

        for input in &self.inputs {
            hasher.update(input.hash.as_bytes());
        }

        hasher.update(self.execution.exit_code.to_string().as_bytes());
        if let Some(stdout_hash) = &self.execution.stdout_hash {
            hasher.update(stdout_hash.as_bytes());
        }
        if let Some(stderr_hash) = &self.execution.stderr_hash {
            hasher.update(stderr_hash.as_bytes());
        }

        for output in &self.outputs {
            hasher.update(output.hash.as_bytes());
        }

        let result = hasher.finalize();
        hex::encode(result)
    }

    /// Check if this reflection represents a successful execution
    pub fn is_success(&self) -> bool {
        self.execution.exit_code == 0
    }
}

/// Hash arbitrary data using SHA256
pub fn hash_data(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Hash a file's contents
pub fn hash_file(path: &std::path::Path) -> std::io::Result<String> {
    let data = std::fs::read(path)?;
    Ok(hash_data(&data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_determinism() {
        let data = b"test data";
        let hash1 = hash_data(data);
        let hash2 = hash_data(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_reflection_id_generation() {
        let envelope = ReflectionEnvelope {
            id: String::new(),
            timestamp: Utc::now(),
            transform: TransformWitness {
                content_hash: "abc123".to_string(),
                source_path: PathBuf::from("test.nu"),
                version: None,
            },
            inputs: vec![],
            outputs: vec![],
            execution: ExecutionMeta {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                stdout_hash: None,
                stderr_hash: None,
                duration_ms: 100,
                witness: "test".to_string(),
            },
        };

        let id = envelope.generate_id();
        assert!(!id.is_empty());
        assert_eq!(id.len(), 64); // SHA256 hex length
    }

    #[test]
    fn test_distinct_executions_no_alias() {
        let base = ReflectionEnvelope {
            id: String::new(),
            timestamp: Utc::now(),
            transform: TransformWitness {
                content_hash: "abc123".to_string(),
                source_path: PathBuf::from("test.nu"),
                version: None,
            },
            inputs: vec![],
            outputs: vec![],
            execution: ExecutionMeta {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                stdout_hash: None,
                stderr_hash: None,
                duration_ms: 100,
                witness: "test".to_string(),
            },
        };

        let failed = ReflectionEnvelope {
            id: String::new(),
            timestamp: Utc::now(),
            transform: TransformWitness {
                content_hash: "abc123".to_string(),
                source_path: PathBuf::from("test.nu"),
                version: None,
            },
            inputs: vec![],
            outputs: vec![],
            execution: ExecutionMeta {
                exit_code: 1,
                stdout: String::new(),
                stderr: String::new(),
                stdout_hash: None,
                stderr_hash: None,
                duration_ms: 100,
                witness: "test".to_string(),
            },
        };

        let id_success = base.generate_id();
        let id_fail = failed.generate_id();
        assert_ne!(id_success, id_fail);
    }

    #[test]
    fn test_distinct_outputs_no_alias() {
        let base = ReflectionEnvelope {
            id: String::new(),
            timestamp: Utc::now(),
            transform: TransformWitness {
                content_hash: "abc123".to_string(),
                source_path: PathBuf::from("test.nu"),
                version: None,
            },
            inputs: vec![],
            outputs: vec![],
            execution: ExecutionMeta {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                stdout_hash: None,
                stderr_hash: None,
                duration_ms: 100,
                witness: "test".to_string(),
            },
        };

        let with_output = ReflectionEnvelope {
            id: String::new(),
            timestamp: Utc::now(),
            transform: TransformWitness {
                content_hash: "abc123".to_string(),
                source_path: PathBuf::from("test.nu"),
                version: None,
            },
            inputs: vec![],
            outputs: vec![OutputArtifact {
                path: PathBuf::from("result.txt"),
                hash: "output_hash".to_string(),
                artifact_type: "text".to_string(),
            }],
            execution: ExecutionMeta {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                stdout_hash: None,
                stderr_hash: None,
                duration_ms: 100,
                witness: "test".to_string(),
            },
        };

        let id_base = base.generate_id();
        let id_output = with_output.generate_id();
        assert_ne!(id_base, id_output);
    }
}
