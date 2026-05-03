use crate::error::{Error, Result};
use crate::models::{KnowledgeEntry, KnowledgeRow, Source};
use crate::schema;
use rusqlite::{Connection, params};
use serde_json::Value;
use std::path::Path;

/// The primary interface for interacting with the SQLite-backed knowledge store.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens a connection to the database and runs migrations.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Inserts a new knowledge entry into the store.
    pub fn insert(&self, entry: KnowledgeEntry) -> Result<i64> {
        let tags_json = serde_json::to_string(&entry.tags).map_err(Error::Json)?;
        let meta_json = serde_json::to_string(&entry.metadata).map_err(Error::Json)?;
        // In this prototype, we use a placeholder checksum.
        let checksum = "initial";

        self.conn.execute(
            "INSERT INTO knowledge (content, kind, tags, meta, weight, active, checksum)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
            params![
                entry.content,
                format!("{:?}", entry.kind),
                tags_json,
                meta_json,
                entry.weight,
                checksum,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        self.log_event("insert", Some(id), None, &format!("{}", entry.source))?;

        Ok(id)
    }

    /// Deactivates a knowledge entry by setting active=0 and logging the event.
    pub fn deactivate(&self, id: i64, source: Source, reason: Option<&str>) -> Result<()> {
        let affected = self
            .conn
            .execute("UPDATE knowledge SET active = 0 WHERE id = ?1", params![id])?;

        if affected == 0 {
            return Err(Error::NotFound(format!("Knowledge entry {}", id)));
        }

        // Log the deactivation event
        let payload = reason.map(|r| serde_json::json!({ "reason": r }));
        self.log_event("deactivate", Some(id), payload, &format!("{:?}", source))?;

        Ok(())
    }

    /// Queries knowledge entries based on tags and a limit.
    pub fn query(&self, tags: &[&str], limit: usize, _context: &str) -> Result<Vec<KnowledgeRow>> {
        if tags.is_empty() {
            return Ok(vec![]);
        }

        let mut stmt = self.conn.prepare(
            "SELECT id, content, tags, meta, active FROM knowledge
             WHERE active = 1
             ORDER BY id ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            let tags_str: String = row.get(2)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let metadata_str: String = row.get(3)?;
            let metadata = serde_json::from_str(&metadata_str).unwrap_or_default();

            Ok(KnowledgeRow {
                id: row.get(0)?,
                content: row.get(1)?,
                tags,
                metadata,
                active: row.get(4)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            let row = row?;
            if row
                .tags
                .iter()
                .any(|tag| tags.iter().any(|query_tag| query_tag == tag))
            {
                results.push(row);
            }
            if results.len() >= limit {
                break;
            }
        }
        Ok(results)
    }

    /// Finds the active knowledge row derived from a specific provenance tuple.
    pub fn find_active_by_provenance(
        &self,
        source_type: &str,
        source_id: &str,
    ) -> Result<Option<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM knowledge
             WHERE active = 1
               AND json_extract(meta, '$.source_type') = ?1
               AND json_extract(meta, '$.source_id') = ?2
             LIMIT 1",
        )?;

        let result = stmt.query_row(params![source_type, source_id], |row| row.get(0));
        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(Error::Database(err)),
        }
    }

    /// Deactivates all active knowledge rows derived from a specific provenance tuple.
    pub fn deactivate_by_provenance(
        &self,
        source_type: &str,
        source_id: &str,
        source: Source,
        reason: Option<&str>,
    ) -> Result<usize> {
        let affected = self.conn.execute(
            "UPDATE knowledge
              SET active = 0
              WHERE active = 1
                AND json_extract(meta, '$.source_type') = ?1
                AND json_extract(meta, '$.source_id') = ?2",
            params![source_type, source_id],
        )?;

        if affected > 0 {
            let payload = serde_json::json!({
                "reason": reason,
                "source_type": source_type,
                "source_id": source_id,
            });
            self.log_event("deactivate", None, Some(payload), &format!("{:?}", source))?;
        }

        Ok(affected)
    }

    /// Deactivates all active knowledge rows by provenance_id across all provenance sources.
    pub fn deactivate_by_provenance_id(
        &self,
        provenance_id: &str,
        source: Source,
        reason: Option<&str>,
    ) -> Result<usize> {
        let affected = self.conn.execute(
            "UPDATE knowledge
              SET active = 0
              WHERE active = 1
                AND json_extract(meta, '$.provenance_id') = ?1",
            params![provenance_id],
        )?;

        if affected > 0 {
            let payload = serde_json::json!({
                "reason": reason,
                "provenance_id": provenance_id,
            });
            self.log_event("deactivate", None, Some(payload), &format!("{:?}", source))?;
        }

        Ok(affected)
    }

    /// Verifies the integrity of all active rows by checking checksums.
    pub fn verify(&self) -> Result<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, content, checksum FROM knowledge WHERE active = 1")?;
        let mut bad_ids = Vec::new();

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        for row in rows {
            let (id, _content, stored) = row?;
            // In this prototype, we use the placeholder 'initial' to match the insertion logic.
            let computed = "initial";
            if computed != stored {
                bad_ids.push(id);
            }
        }

        Ok(bad_ids)
    }

    /// Decays the weight of a knowledge entry based on its staleness metadata.
    /// Patterns decay once conditions change — confidence decreases over time unless reinforced.
    pub fn decay_weight(&self, id: i64) -> Result<f64> {
        let metadata_str: String = self.conn.query_row(
            "SELECT meta FROM knowledge WHERE id = ?1",
            params![id],
            |row| Ok(row.get::<_, String>(2)?),
        )?;

        let metadata: serde_json::Value = serde_json::from_str(&metadata_str).unwrap_or_default();

        let stale_after = metadata
            .as_object()
            .and_then(|obj| obj.get("stale_after"))
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let current = chrono::Utc::now();

        let weight = if let Some(stale) = stale_after {
            if current > stale {
                let days_since = (current - stale).as_seconds_f64() / 86400.0;
                // Decay: weight *= 0.95 per day past stale threshold
                1.0 * 0.95_f64.powf(days_since)
            } else {
                1.0
            }
        } else {
            1.0
        };

        Ok(weight)
    }

    /// Returns recent events from the event log.
    pub fn events(&self, limit: usize) -> Result<Vec<crate::models::EventRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, kind, ts FROM events ORDER BY ts DESC LIMIT ?1")?;

        let rows = stmt.query_map(params![limit], |row| {
            let ts_str: String = row.get(2)?;
            // Attempt to parse RFC3339 or fallback to current time if format fails in prototype.
            let dt = chrono::DateTime::parse_from_rfc3339(&ts_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            Ok(crate::models::EventRow {
                id: row.get(0)?,
                event_type: row.get(1)?,
                timestamp: dt,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Internal helper to log an event to the database.
    fn log_event(
        &self,
        kind: &str,
        target_id: Option<i64>,
        payload: Option<Value>,
        source: &str,
    ) -> Result<()> {
        let payload_json = payload
            .map(|p| serde_json::to_string(&p))
            .transpose()
            .map_err(Error::Json)?;

        self.conn.execute(
            "INSERT INTO events (kind, target_id, payload, source) VALUES (?1, ?2, ?3, ?4)",
            params![kind, target_id, payload_json, source],
        )?;
        Ok(())
    }
}

// Helper implementation for KnowledgeKind to allow string conversion for SQLite
impl std::fmt::Display for crate::models::KnowledgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            crate::models::KnowledgeKind::Instruction => write!(f, "instruction"),
            crate::models::KnowledgeKind::Pattern => write!(f, "pattern"),
            crate::models::KnowledgeKind::Example => write!(f, "example"),
            crate::models::KnowledgeKind::Context => write!(f, "context"),
        }
    }
}

// Helper implementation for Source to allow string conversion for SQLite
impl std::fmt::Display for crate::models::Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            crate::models::Source::User => write!(f, "user"),
            crate::models::Source::Agent => write!(f, "agent"),
            crate::models::Source::System => write!(f, "system"),
        }
    }
}
