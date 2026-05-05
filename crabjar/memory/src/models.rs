use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeKind {
    Instruction,
    Pattern,
    Example,
    Context,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    User,
    Agent,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub content: String,
    pub kind: KnowledgeKind,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
    pub weight: f64,
    pub source: Source,
}

impl KnowledgeEntry {
    pub fn new(content: impl Into<String>, kind: KnowledgeKind) -> Self {
        Self {
            content: content.into(),
            kind,
            tags: Vec::new(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            weight: 1.0,
            source: Source::User,
        }
    }

    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    pub fn weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    pub fn meta<V: serde::Serialize>(mut self, key: impl Into<String>, value: V) -> Self {
        if let Ok(val) = serde_json::to_value(value)
            && let Some(obj) = self.metadata.as_object_mut() {
            obj.insert(key.into(), val);
        }
        self
    }

    pub fn stale(mut self, after: chrono::DateTime<Utc>) -> Self {
        if let Ok(val) = serde_json::to_value(after)
            && let Some(obj) = self.metadata.as_object_mut() {
            obj.insert("stale_after".into(), val);
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRow {
    pub id: i64,
    pub content: String,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRow {
    pub id: i64,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventKind {
    pub kind: String,
    pub target_id: Option<i64>,
    pub payload: Option<serde_json::Value>,
    pub source: String,
    pub ts: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EventType {
    Insert,
    Deactivate,
    Query,
    Promote,
}
