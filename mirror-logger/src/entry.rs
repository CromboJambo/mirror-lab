use crate::tri::Tri;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorEntry {
    pub id: u64,
    pub input: String,
    pub state: Tri,
    pub reason: Option<String>,
    pub timestamp: u64,
    pub tags: Vec<String>,
    pub parent: Option<u64>,
}

impl MirrorEntry {
    pub fn new(input: String, state: Tri) -> Self {
        Self {
            id: 0, // Will be set by store
            input,
            state,
            reason: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            tags: Vec::new(),
            parent: None,
        }
    }

    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = Some(reason);
        self
    }

    pub fn with_parent(mut self, parent: u64) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn with_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }
}
