// crabjar/src/knowledge_store/mod.rs
// Bridge between state-docs and knowledge store

pub mod commands;

use agent_context::{KnowledgeEntry, KnowledgeKind, Source, Store};
use rusqlite::Connection;
use serde_json::json;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::state_docs::{AnnotationEntry, AnnotationKind, StateDocsManager};

/// Helper to produce a standard knowledge-response JSON object
pub fn knowledge_response(
    message: impl Into<String>,
    payload: serde_json::Value,
) -> serde_json::Value {
    let mut response = json!({
        "success": true,
        "message": message.into(),
        "payload": payload,
    });

    if let Some(payload_obj) = response["payload"].as_object().cloned() {
        if let Some(response_obj) = response.as_object_mut() {
            for (key, value) in payload_obj {
                response_obj.insert(key, value);
            }
        }
    }

    response
}

/// Bridge between state-docs and knowledge store
pub struct KnowledgeBridge<'a> {
    knowledge_store: Store,
    state_docs: StateDocsManager<'a>,
    mirror_log_conn: Option<Connection>,
}

impl<'a> KnowledgeBridge<'a> {
    const STATE_DOC_SOURCE_TYPE: &'static str = "state_doc_annotation";
    const MIRROR_LOG_SOURCE_TYPE: &'static str = "mirror_log_event";

    pub fn new(
        knowledge_store_path: &str,
        project_root: impl Into<PathBuf>,
        mirror_log_db_path: Option<PathBuf>,
    ) -> Result<Self, agent_context::Error> {
        let knowledge_store = Store::open(knowledge_store_path)?;
        let state_docs = StateDocsManager::new(project_root);
        let mirror_log_conn = mirror_log_db_path
            .map(|path| Connection::open(path))
            .transpose()?;

        Ok(Self {
            knowledge_store,
            state_docs,
            mirror_log_conn,
        })
    }

    /// Convert state-docs annotation to knowledge entry
    pub fn annotation_to_knowledge(
        &self,
        annotation: &AnnotationEntry,
    ) -> Result<KnowledgeEntry, agent_context::Error> {
        let kind = match annotation.kind {
            AnnotationKind::Note => KnowledgeKind::Context,
            AnnotationKind::Question => KnowledgeKind::Instruction,
        };

        let confidence = annotation_confidence(annotation);
        let provenance_id = Uuid::new_v4().to_string();
        let mut entry = KnowledgeEntry::new(&annotation.message, kind)
            .meta("source_type", Self::STATE_DOC_SOURCE_TYPE)
            .meta("source_id", &annotation.id)
            .meta("source_doc", &annotation.doc)
            .meta(
                "annotation_kind",
                format!("{:?}", annotation.kind).to_lowercase(),
            )
            .meta("confidence", confidence)
            .meta("derived_at_unix_ms", now_unix_ms())
            .meta("status", "active")
            .meta("provenance_id", provenance_id)
            .meta("provenance_source", Self::STATE_DOC_SOURCE_TYPE)
            .meta("provenance_set_at_unix_ms", now_unix_ms());
        entry.source = Source::Agent;
        entry.weight = confidence;
        entry.tags = std::iter::once("state-doc".to_string())
            .chain(annotation.doc.split('_').map(|s| s.to_string()))
            .collect();
        Ok(entry)
    }

    /// Query knowledge entries by tags
    pub fn query_state_docs(
        &self,
        tags: &[&str],
        limit: usize,
    ) -> Result<Vec<serde_json::Value>, agent_context::Error> {
        let rows = self.knowledge_store.query(tags, limit, "")?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let mut meta = row.metadata;
                if let Some(source_id) = meta.get("source_id").cloned() {
                    if let Some(meta_obj) = meta.as_object_mut() {
                        meta_obj
                            .entry("annotation_id".to_string())
                            .or_insert(source_id);
                    }
                }
                json!({
                    "id": row.id,
                    "content": row.content,
                    "tags": row.tags,
                    "meta": meta.clone(),
                    "metadata": meta,
                    "active": row.active,
                })
            })
            .collect())
    }

    /// Sync all open annotations for a state-doc into the knowledge store
    pub fn sync_state_doc_annotations(
        &self,
        doc_name: &str,
    ) -> Result<Vec<i64>, agent_context::Error> {
        let overlay = self
            .state_docs
            .load_overlay_for_path(&self.state_docs.resolve_doc_path(doc_name)?)?;

        let mut ids = Vec::new();
        for entry in overlay.entries {
            if entry.status == crate::state_docs::AnnotationStatus::Open {
                if self
                    .knowledge_store
                    .find_active_by_provenance(Self::STATE_DOC_SOURCE_TYPE, &entry.id)?
                    .is_some()
                {
                    continue;
                }
                let knowledge = self.annotation_to_knowledge(&entry)?;
                let id = self.knowledge_store.insert(knowledge)?;
                ids.push(id);
            }
        }

        Ok(ids)
    }

    /// List all state-docs that have synced annotations in the knowledge store
    pub fn list_synced_state_docs(&self) -> Result<Vec<String>, agent_context::Error> {
        let docs = self.state_docs.list_docs()?;
        let mut synced = Vec::new();
        for summary in docs {
            let overlay = self
                .state_docs
                .load_overlay_for_path(&self.state_docs.resolve_doc_path(&summary.doc)?)?;
            if !overlay.entries.is_empty() {
                synced.push(summary.doc);
            }
        }
        Ok(synced)
    }

    /// Get knowledge entries associated with a specific state-doc
    pub fn get_state_doc_knowledge(
        &self,
        doc_name: &str,
    ) -> Result<Vec<serde_json::Value>, agent_context::Error> {
        let overlay = self
            .state_docs
            .load_overlay_for_path(&self.state_docs.resolve_doc_path(doc_name)?)?;

        let tags: Vec<&str> = overlay.entries.iter().map(|e| e.doc.as_str()).collect();

        self.query_state_docs(&tags, 100)
    }

    /// Get recent events from the knowledge store's event log
    pub fn get_events(&self, limit: usize) -> Result<Vec<serde_json::Value>, agent_context::Error> {
        let rows = self.knowledge_store.events(limit)?;
        Ok(rows
            .into_iter()
            .map(
                |row| json!({ "id": row.id, "event_type": row.event_type, "timestamp": row.timestamp }),
            )
            .collect())
    }

    /// Deactivates knowledge derived from a resolved annotation.
    pub fn deactivate_annotation_knowledge(
        &self,
        annotation_id: &str,
        reason: Option<&str>,
    ) -> Result<usize, agent_context::Error> {
        self.knowledge_store.deactivate_by_provenance(
            Self::STATE_DOC_SOURCE_TYPE,
            annotation_id,
            Source::Agent,
            reason,
        )
    }

    /// Deactivates knowledge derived from a resolved annotation entry.
    pub fn deactivate_resolved_annotation_knowledge(
        &self,
        resolved: &AnnotationEntry,
        reason: Option<&str>,
    ) -> Result<usize, agent_context::Error> {
        self.knowledge_store.deactivate_by_provenance(
            Self::STATE_DOC_SOURCE_TYPE,
            &resolved.id,
            Source::Agent,
            reason,
        )
    }

    /// Deactivates all knowledge entries by provenance_id across all provenance sources.
    pub fn deactivate_by_provenance_id(
        &self,
        provenance_id: &str,
        reason: Option<&str>,
    ) -> Result<usize, agent_context::Error> {
        self.knowledge_store.deactivate_by_provenance_id(
            provenance_id,
            Source::Agent,
            reason,
        )
    }

    /// Promote a raw event from mirror-log to a knowledge entry
    pub fn promote_event(&self, event_id: i64) -> Result<String, agent_context::Error> {
        let conn = self.mirror_log_conn.as_ref().ok_or_else(|| {
            agent_context::Error::Internal("mirror-log connection not available".to_string())
        })?;

        let id_str = event_id.to_string();

        let (content, _source, meta): (String, String, Option<String>) = conn
            .query_row(
                "SELECT content, source, meta FROM events WHERE id = ?1",
                [&id_str],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|e| agent_context::Error::Internal(format!("Failed to find event: {}", e)))?;

        let provenance_id = Uuid::new_v4().to_string();
        let mut entry = KnowledgeEntry::new(&content, KnowledgeKind::Context);
        entry.source = Source::Agent;
        entry = entry
            .meta("source_type", Self::MIRROR_LOG_SOURCE_TYPE)
            .meta("source_id", event_id)
            .meta("confidence", 0.85)
            .meta("derived_at_unix_ms", now_unix_ms())
            .meta("status", "active")
            .meta("provenance_id", provenance_id)
            .meta("provenance_source", Self::MIRROR_LOG_SOURCE_TYPE)
            .meta("provenance_set_at_unix_ms", now_unix_ms());
        if let Some(m) = meta {
            entry = entry.meta("event-meta", json!(m));
        }

        let new_id = self.knowledge_store.insert(entry)?;
        Ok(new_id.to_string())
    }
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn annotation_confidence(annotation: &AnnotationEntry) -> f64 {
    let base = match annotation.kind {
        AnnotationKind::Note => 0.80,
        AnnotationKind::Question => 0.55,
    };

    let message = annotation.message.to_ascii_lowercase();
    let mut confidence: f64 = base;

    for marker in [
        "maybe",
        "might",
        "should",
        "todo",
        "follow-up",
        "follow up",
        "?",
    ] {
        if message.contains(marker) {
            confidence -= 0.10;
        }
    }

    confidence.clamp(0.20, 0.95)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_docs::AnnotationStatus;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn sync_is_idempotent_on_open_annotations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("knowledge.db");
        let db_path_str = db_path.to_string_lossy().into_owned();
        let bridge = KnowledgeBridge::new(&db_path_str, dir.path(), None).unwrap();

        let docs_dir = dir.path().join("state-docs");
        fs::create_dir_all(docs_dir.join("overlay")).unwrap();
        fs::write(docs_dir.join("alpha.md"), "# Alpha\n").unwrap();
        fs::write(
            docs_dir.join("overlay").join("alpha.overlay.json"),
            r#"{
  "entries": [
    {
      "id": "alpha-md-123-0",
      "kind": "note",
      "message": "Keep this",
      "author": "agent",
      "doc": "alpha.md",
      "line": null,
      "status": "open",
      "created_at_unix_ms": 123
    }
  ]
}"#,
        )
        .unwrap();

        let first_ids = bridge.sync_state_doc_annotations("alpha").unwrap();
        assert_eq!(first_ids.len(), 1);

        let second_ids = bridge.sync_state_doc_annotations("alpha").unwrap();
        assert_eq!(second_ids.len(), 0);
    }

    #[test]
    fn deactivate_resolved_annotation_knowledge_returns_count() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("knowledge.db");
        let db_path_str = db_path.to_string_lossy().into_owned();
        let bridge = KnowledgeBridge::new(&db_path_str, dir.path(), None).unwrap();

        let docs_dir = dir.path().join("state-docs");
        fs::create_dir_all(docs_dir.join("overlay")).unwrap();
        fs::write(docs_dir.join("beta.md"), "# Beta\n").unwrap();
        fs::write(
            docs_dir.join("overlay").join("beta.overlay.json"),
            r#"{
  "entries": [
    {
      "id": "beta-md-456-0",
      "kind": "question",
      "message": "Decided yes",
      "author": "agent",
      "doc": "beta.md",
      "line": null,
      "status": "open",
      "created_at_unix_ms": 456
    }
  ]
}"#,
        )
        .unwrap();

        let ids = bridge.sync_state_doc_annotations("beta").unwrap();
        assert_eq!(ids.len(), 1);

        let resolved_entry = AnnotationEntry {
            id: "beta-md-456-0".to_string(),
            kind: AnnotationKind::Question,
            message: "Decided yes".to_string(),
            author: "agent".to_string(),
            doc: "beta.md".to_string(),
            line: None,
            status: AnnotationStatus::Resolved,
            created_at_unix_ms: 456,
        };

        let deactivated = bridge
            .deactivate_resolved_annotation_knowledge(&resolved_entry, Some("answered"))
            .unwrap();
        assert_eq!(deactivated, 1);

        let rows = bridge.query_state_docs(&["state-doc", "beta"], 100).unwrap();
        assert_eq!(rows.len(), 0);
    }
}
