use chrono::{DateTime, TimeZone, Utc};
use std::fs;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StagedEvent {
    pub id: String,
    pub source: String,
    pub content: String,
    pub meta: Option<String>,
    pub timestamp: i64,
}

impl StagedEvent {
    pub fn new(source: &str, content: &str, meta: Option<&str>) -> Self {
        let now = Utc::now().timestamp();
        let id = Uuid::new_v4().to_string();

        Self {
            id,
            source: source.to_string(),
            content: content.to_string(),
            meta: meta.map(|s| s.to_string()),
            timestamp: now,
        }
    }

    pub fn with_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }

    /// Return the event timestamp as a `DateTime<Utc>`.
    pub fn timestamp_utc(&self) -> DateTime<Utc> {
        Utc.timestamp_opt(self.timestamp, 0).unwrap()
    }

    /// Persist this staged event to disk as a JSON file.
    pub fn save_to_file(&self, staging_dir: &Path) -> Result<(), std::io::Error> {
        let filename = format!("{}.json", self.id);
        let path = staging_dir.join(filename);

        fs::create_dir_all(staging_dir)?;
        let json = serde_json::to_string_pretty(self)?;

        fs::write(path, json)?;
        Ok(())
    }

    /// Load a staged event by its ID from the staging directory.
    pub fn load_from_file(id: &str, staging_dir: &Path) -> Result<Self, std::io::Error> {
        let filename = format!("{}.json", id);
        let path = staging_dir.join(filename);

        let json = fs::read_to_string(path)?;
        let event: StagedEvent = serde_json::from_str(&json)?;
        Ok(event)
    }

    /// Load a staged event from an arbitrary file path.
    pub fn from_file(path: &Path) -> Result<Self, std::io::Error> {
        let json = fs::read_to_string(path)?;
        let event: StagedEvent = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(event)
    }

    /// Load all staged events from a directory.
    pub fn load_all(staging_dir: &Path) -> Result<Vec<Self>, std::io::Error> {
        let mut events = Vec::new();

        if !staging_dir.exists() {
            return Ok(events);
        }

        for entry in fs::read_dir(staging_dir)? {
            let path = entry?.path();
            if path.extension() == Some(std::ffi::OsStr::new("json")) {
                match Self::from_file(&path) {
                    Ok(event) => events.push(event),
                    Err(e) => eprintln!("Failed to parse staging event {}: {}", path.display(), e),
                }
            }
        }

        events.sort_by_key(|e| e.timestamp);
        Ok(events)
    }

    /// Remove the staging file for this event.
    pub fn remove_file(&self, staging_dir: &Path) -> Result<(), std::io::Error> {
        let filename = format!("{}.json", self.id);
        let path = staging_dir.join(filename);
        fs::remove_file(path)
    }
}
