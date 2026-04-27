use crate::reflection::ReflectionEnvelope;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Append-only ledger of all reflections
pub struct Ledger {
    /// Base directory for all ledger data
    base_path: PathBuf,

    /// Path to the append-only journal file
    journal_path: PathBuf,
}

/// A single ledger entry - one line in the journal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Reflection ID
    pub reflection_id: String,

    /// When this entry was added to the ledger
    pub ledger_time: chrono::DateTime<chrono::Utc>,

    /// Path to the reflection envelope
    pub envelope_path: PathBuf,

    /// Pipeline source path
    pub pipeline: String,

    /// Success or failure
    pub success: bool,
}

impl Ledger {
    /// Create or open a ledger at the given path
    pub fn new(base_path: impl AsRef<Path>) -> std::io::Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();

        // Create directory structure
        std::fs::create_dir_all(&base_path)?;
        std::fs::create_dir_all(base_path.join("reflections"))?;
        std::fs::create_dir_all(base_path.join("artifacts"))?;

        let journal_path = base_path.join("ledger.jsonl");

        // Create journal if it doesn't exist
        if !journal_path.exists() {
            File::create(&journal_path)?;
        }

        Ok(Self {
            base_path,
            journal_path,
        })
    }

    /// Append a reflection to the ledger (the ONLY write operation)
    pub fn append(&self, envelope: &mut ReflectionEnvelope) -> std::io::Result<String> {
        let reflection_id = envelope.generate_id();
        envelope.id = reflection_id.clone();

        // Store the envelope in a content-addressed location
        let envelope_dir = self
            .base_path
            .join("reflections")
            .join(&reflection_id[..2]) // First 2 chars for sharding
            .join(&reflection_id);

        std::fs::create_dir_all(&envelope_dir)?;

        // Write envelope metadata
        let meta_path = envelope_dir.join("meta.json");
        let meta_json = serde_json::to_string_pretty(&envelope)?;
        std::fs::write(&meta_path, meta_json)?;

        // Write outputs to artifact storage
        for output in &envelope.outputs {
            let artifact_path = self
                .base_path
                .join("artifacts")
                .join(&output.hash[..2])
                .join(&output.hash);

            if let Some(parent) = artifact_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Copy artifact to content-addressed storage
            if output.path.exists() {
                std::fs::copy(&output.path, &artifact_path)?;
            }
        }

        // Append to the journal (append-only!)
        let entry = LedgerEntry {
            reflection_id: reflection_id.clone(),
            ledger_time: chrono::Utc::now(),
            envelope_path: meta_path,
            pipeline: envelope.transform.source_path.display().to_string(),
            success: envelope.is_success(),
        };

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.journal_path)?;

        let entry_json = serde_json::to_string(&entry)?;
        writeln!(file, "{}", entry_json)?;

        Ok(reflection_id)
    }

    /// Read all ledger entries (chronological order)
    ///
    /// Note: This is deliberately corruption-tolerant. Append-only ledgers must
    /// be resilient to partial failures - one bad line should not brick the entire
    /// history. We log warnings but continue reading.
    pub fn read_all(&self) -> std::io::Result<Vec<LedgerEntry>> {
        let file = File::open(&self.journal_path)?;
        let reader = BufReader::new(file);

        let mut entries = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                match serde_json::from_str::<LedgerEntry>(&line) {
                    Ok(entry) => entries.push(entry),
                    Err(e) => {
                        eprintln!("Warning: corrupted ledger entry: {}", e);
                        // Continue reading - don't fail on corrupt entries
                    }
                }
            }
        }

        Ok(entries)
    }

    /// Get a specific reflection envelope by ID
    pub fn get_reflection(&self, id: &str) -> std::io::Result<ReflectionEnvelope> {
        let meta_path = self
            .base_path
            .join("reflections")
            .join(&id[..2])
            .join(id)
            .join("meta.json");

        let meta_json = std::fs::read_to_string(meta_path)?;
        let envelope = serde_json::from_str(&meta_json)?;

        Ok(envelope)
    }

    /// List recent reflections (newest first)
    pub fn list_recent(&self, limit: usize) -> std::io::Result<Vec<LedgerEntry>> {
        let mut entries = self.read_all()?;
        entries.reverse(); // Newest first
        entries.truncate(limit);
        Ok(entries)
    }

    /// Get the base path of this ledger
    #[allow(dead_code)]
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reflection::*;
    use tempfile::TempDir;

    #[test]
    fn test_ledger_creation() {
        let tmp = TempDir::new().unwrap();
        let ledger = Ledger::new(tmp.path()).unwrap();

        assert!(ledger.journal_path.exists());
        assert!(tmp.path().join("reflections").exists());
        assert!(tmp.path().join("artifacts").exists());
    }

    #[test]
    fn test_append_and_retrieve() {
        let tmp = TempDir::new().unwrap();
        let ledger = Ledger::new(tmp.path()).unwrap();

        let mut envelope = ReflectionEnvelope {
            id: String::new(),
            timestamp: chrono::Utc::now(),
            transform: TransformWitness {
                content_hash: "test123".to_string(),
                source_path: PathBuf::from("test.nu"),
                version: None,
            },
            inputs: vec![],
            outputs: vec![],
            execution: ExecutionMeta {
                exit_code: 0,
                stdout: "success".to_string(),
                stderr: String::new(),
                stdout_hash: None,
                stderr_hash: None,
                duration_ms: 50,
                witness: "test".to_string(),
            },
        };

        let id = ledger.append(&mut envelope).unwrap();
        let retrieved = ledger.get_reflection(&id).unwrap();

        assert_eq!(retrieved.transform.content_hash, "test123");
    }
}
