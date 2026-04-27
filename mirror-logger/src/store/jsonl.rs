use crate::entry::MirrorEntry;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// JSONL append-only store for mirror-log
#[allow(dead_code)]
pub struct JsonlStore {
    path: PathBuf,
    writer: BufWriter<File>,
}

#[derive(Debug)]
pub enum JsonlError {
    IoError(std::io::Error),
    JsonError(serde_json::Error),
}

impl From<std::io::Error> for JsonlError {
    fn from(err: std::io::Error) -> Self {
        JsonlError::IoError(err)
    }
}

impl From<serde_json::Error> for JsonlError {
    fn from(err: serde_json::Error) -> Self {
        JsonlError::JsonError(err)
    }
}

impl std::fmt::Display for JsonlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonlError::IoError(e) => write!(f, "IO error: {}", e),
            JsonlError::JsonError(e) => write!(f, "JSON error: {}", e),
        }
    }
}

impl std::error::Error for JsonlError {}

impl JsonlStore {
    /// Create a new JSONL store
    pub fn new(path: impl AsRef<Path>) -> Result<Self, JsonlError> {
        let path = path.as_ref().to_path_buf();

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Open file in append mode
        let file = File::options().create(true).append(true).open(&path)?;

        Ok(Self {
            path,
            writer: BufWriter::new(file),
        })
    }

    /// Append an entry to the JSONL file
    pub fn append(&mut self, entry: &MirrorEntry) -> Result<(), JsonlError> {
        let line = serde_json::to_string(entry)?;

        writeln!(self.writer, "{}", line)?;
        self.writer.flush()?;

        Ok(())
    }

    /// Get the file path
    #[allow(dead_code)]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Read all lines from the JSONL file
    #[allow(dead_code)]
    pub fn read_all(&self) -> Result<Vec<MirrorEntry>, JsonlError> {
        let content = fs::read_to_string(&self.path)?;

        content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).map_err(JsonlError::JsonError))
            .collect()
    }

    /// Get line count
    #[allow(dead_code)]
    pub fn line_count(&self) -> Result<usize, JsonlError> {
        let content = fs::read_to_string(&self.path)?;
        Ok(content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count())
    }
}
