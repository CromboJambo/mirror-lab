//! Statefulness primitives: session/task scoping, provenance tracking, and a
//! memory-context retrieval API for agents.
//!
//! Design constraints (see AGENTS.md — Non-Negotiable Architectural Constraints):
//!
//! - **Append-only.** Sessions and provenance entries are never mutated in
//!   place. A change is a new entry; old entries remain queryable.
//! - **Provenance is immutable.** Every derived output or configurable
//!   baseline gets a UUID + provenance entry (`set_at`, `reason`, `source`).
//!   Superseding a value means inserting a new entry, not overwriting.
//! - **Every abstraction carries its own doubt.** `MemoryContext` includes a
//!   `doubt` block stating what it might have missed, what assumptions it
//!   made, where it might break, and how stale it is.

use rusqlite::{Connection, Error as SqlError, Result, params};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::view::Event;

/// Errors from the state layer.
#[derive(Debug, Error)]
pub enum StateError {
    #[error("database error: {0}")]
    Db(#[from] SqlError),

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("event not found: {0}")]
    EventNotFound(String),
}

/// A session (or task) that scopes a run of events.
#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub id: String,
    pub source: String,
    pub summary: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
}

/// An immutable provenance entry anchoring a derived output or baseline value.
#[derive(Debug, Clone, Serialize)]
pub struct ProvenanceEntry {
    pub id: String,
    /// What kind of thing this anchors: "event", "baseline", "reflection", ...
    pub subject_kind: String,
    /// Identifier of the anchored subject (event id, baseline key, ...).
    pub subject_id: String,
    /// Why this entry exists (the reason for the value/merge).
    pub reason: String,
    /// Where it came from (agent, user, daemon, model name, ...).
    pub source: String,
    /// Optional reference to the raw event this was derived from.
    pub event_id: Option<String>,
    pub set_at: i64,
}

/// The doubt block: every derived output must state its own uncertainty.
#[derive(Debug, Clone, Serialize)]
pub struct Doubt {
    /// What this context might have missed.
    pub might_have_missed: String,
    /// What assumptions were made building it.
    pub assumptions: String,
    /// Where it might break.
    pub might_break: String,
    /// How stale it is (relative to the newest event included).
    pub staleness_secs: i64,
}

/// A memory-context bundle for an agent: scoped events + provenance + doubt.
#[derive(Debug, Clone, Serialize)]
pub struct MemoryContext {
    pub events: Vec<Event>,
    pub session: Option<Session>,
    pub provenance: Vec<ProvenanceEntry>,
    pub doubt: Doubt,
}

fn unix_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Create a new session and return it.
pub fn create_session(
    conn: &Connection,
    source: &str,
    summary: Option<&str>,
) -> Result<Session, StateError> {
    let id = Uuid::new_v4().to_string();
    let now = unix_now_secs();
    conn.execute(
        "INSERT INTO sessions (id, source, summary, started_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, source, summary, now],
    )?;
    Ok(Session {
        id,
        source: source.to_string(),
        summary: summary.map(|s| s.to_string()),
        started_at: now,
        ended_at: None,
    })
}

/// Close a session (append-only: sets `ended_at` via a new timestamp, the
/// session row is the only mutable state in this module and only in this
/// one field — it carries no content, only a lifecycle marker).
pub fn end_session(conn: &Connection, session_id: &str) -> Result<Session, StateError> {
    let now = unix_now_secs();
    let rows = conn.execute(
        "UPDATE sessions SET ended_at = ?2 WHERE id = ?1 AND ended_at IS NULL",
        params![session_id, now],
    )?;
    if rows == 0 {
        match conn.query_row(
            "SELECT id, source, summary, started_at, ended_at FROM sessions WHERE id = ?1",
            params![session_id],
            |row| {
                Ok(Session {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    summary: row.get(2)?,
                    started_at: row.get(3)?,
                    ended_at: row.get(4)?,
                })
            },
        ) {
            Ok(existing) => Ok(existing),
            Err(_) => Err(StateError::SessionNotFound(session_id.to_string())),
        }
    } else {
        get_session(conn, session_id)
    }
}

/// List sessions, newest first.
pub fn list_sessions(conn: &Connection, limit: i64) -> Result<Vec<Session>, StateError> {
    let mut stmt = conn.prepare(
        "SELECT id, source, summary, started_at, ended_at
         FROM sessions
         ORDER BY started_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok(Session {
            id: row.get(0)?,
            source: row.get(1)?,
            summary: row.get(2)?,
            started_at: row.get(3)?,
            ended_at: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StateError::from)
}

/// Fetch a session by id.
pub fn get_session(conn: &Connection, session_id: &str) -> Result<Session, StateError> {
    conn.query_row(
        "SELECT id, source, summary, started_at, ended_at FROM sessions WHERE id = ?1",
        params![session_id],
        |row| {
            Ok(Session {
                id: row.get(0)?,
                source: row.get(1)?,
                summary: row.get(2)?,
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
            })
        },
    )
    .map_err(|_| StateError::SessionNotFound(session_id.to_string()))
}

/// List events belonging to a session, newest first.
pub fn events_in_session(
    conn: &Connection,
    session_id: &str,
    limit: Option<i64>,
) -> Result<Vec<Event>, StateError> {
    get_session(conn, session_id)?; // validate session exists
    let sql = "SELECT e.id, e.timestamp, e.source, e.content, e.meta, e.ingested_at, e.content_hash
               FROM events e
               JOIN event_sessions es ON es.event_id = e.id
               WHERE es.session_id = ?1
               ORDER BY e.timestamp DESC";
    let sql = match limit {
        Some(n) => format!("{} LIMIT {}", sql, n),
        None => sql.to_string(),
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![session_id], |row| {
        Ok(Event {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            source: row.get(2)?,
            content: row.get(3)?,
            meta: row.get(4)?,
            ingested_at: row.get(5)?,
            content_hash: row.get(6)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StateError::from)
}

/// Attach an existing event to a session (idempotent).
pub fn attach_event_to_session(
    conn: &Connection,
    session_id: &str,
    event_id: &str,
) -> Result<(), StateError> {
    get_session(conn, session_id)?;
    let exists: i64 = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM events WHERE id = ?1)",
            params![event_id],
            |row| row.get(0),
        )
        .map_err(StateError::from)?;
    if exists == 0 {
        return Err(StateError::EventNotFound(event_id.to_string()));
    }
    conn.execute(
        "INSERT OR IGNORE INTO event_sessions (session_id, event_id) VALUES (?1, ?2)",
        params![session_id, event_id],
    )?;
    Ok(())
}

/// Record a new immutable provenance entry. Superseding a value means
/// calling this again — the old entry remains.
pub fn record_provenance(
    conn: &Connection,
    subject_kind: &str,
    subject_id: &str,
    reason: &str,
    source: &str,
    event_id: Option<&str>,
) -> Result<ProvenanceEntry, StateError> {
    let id = Uuid::new_v4().to_string();
    let set_at = unix_now_secs();
    conn.execute(
        "INSERT INTO provenance (id, subject_kind, subject_id, reason, source, event_id, set_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            subject_kind,
            subject_id,
            reason,
            source,
            event_id,
            set_at
        ],
    )?;
    Ok(ProvenanceEntry {
        id,
        subject_kind: subject_kind.to_string(),
        subject_id: subject_id.to_string(),
        reason: reason.to_string(),
        source: source.to_string(),
        event_id: event_id.map(|s| s.to_string()),
        set_at,
    })
}

/// All provenance entries for a subject, oldest first (the lineage).
pub fn provenance_for(
    conn: &Connection,
    subject_kind: &str,
    subject_id: &str,
) -> Result<Vec<ProvenanceEntry>, StateError> {
    let mut stmt = conn.prepare(
        "SELECT id, subject_kind, subject_id, reason, source, event_id, set_at
         FROM provenance
         WHERE subject_kind = ?1 AND subject_id = ?2
         ORDER BY set_at ASC",
    )?;
    let rows = stmt.query_map(params![subject_kind, subject_id], |row| {
        Ok(ProvenanceEntry {
            id: row.get(0)?,
            subject_kind: row.get(1)?,
            subject_id: row.get(2)?,
            reason: row.get(3)?,
            source: row.get(4)?,
            event_id: row.get(5)?,
            set_at: row.get(6)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StateError::from)
}

/// Build a memory-context bundle for an agent.
///
/// Includes: recent events (global recency), all events scoped to `session`
/// when given (deduped against the recent set), and provenance entries
/// anchored to any included event. Always includes a `doubt` block.
pub fn build_context(
    conn: &Connection,
    session_id: Option<&str>,
    limit: i64,
) -> Result<MemoryContext, StateError> {
    let recent = crate::view::recent(conn, limit)?;

    let session = match session_id {
        Some(sid) => Some(get_session(conn, sid)?),
        None => None,
    };

    let mut events = recent;
    let mut seen: std::collections::HashSet<String> = events.iter().map(|e| e.id.clone()).collect();

    if let Some(sid) = session_id {
        for event in events_in_session(conn, sid, None)? {
            if seen.insert(event.id.clone()) {
                events.push(event);
            }
        }
    }

    // Provenance anchored to any included event.
    let mut provenance = Vec::new();
    for event in &events {
        for entry in provenance_for(conn, "event", &event.id)? {
            provenance.push(entry);
        }
    }
    provenance.sort_by_key(|p| p.set_at);

    let newest = events.iter().map(|e| e.timestamp).max().unwrap_or(0);
    let staleness = (unix_now_secs() - newest).max(0);

    let shadowed: i64 = conn
        .query_row("SELECT COUNT(*) FROM shadow_state", [], |row| row.get(0))
        .unwrap_or(0);

    let (might_have_missed, might_break) = match session_id {
        Some(sid) => (
            format!(
                "{} shadowed event(s) are excluded from all queries; nothing is known about events \
                 outside the {} most recent plus session {}'s scope",
                shadowed, limit, sid
            ),
            format!(
                "breaks if session {sid} is closed and new work continues without a new session, \
                 or if events were ingested without session scoping"
            ),
        ),
        None => (
            format!(
                "{} shadowed event(s) are excluded from all queries; nothing is known about events \
                 older than the {} most recent",
                shadowed, limit
            ),
            "breaks if the caller assumed session scoping — no session was requested, so events \
             from all sessions are mixed"
                .to_string(),
        ),
    };

    Ok(MemoryContext {
        events,
        session,
        provenance,
        doubt: Doubt {
            might_have_missed,
            assumptions: "recency-only retrieval (no semantic search); events are surfaced as \
                          stored, without interpretation"
                .to_string(),
            might_break,
            staleness_secs: staleness,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::log::append;
    use tempfile::NamedTempFile;

    fn test_db() -> (NamedTempFile, Connection) {
        let file = NamedTempFile::new().expect("tempfile");
        let conn = init_db(file.path()).expect("init_db");
        (file, conn)
    }

    #[test]
    fn test_session_scopes_events() {
        let (_file, conn) = test_db();

        let session = create_session(&conn, "agent", Some("schema fix session")).unwrap();
        let e1 = append(&conn, "cli", "event in session", None).unwrap();
        let e2 = append(&conn, "cli", "event outside session", None).unwrap();

        attach_event_to_session(&conn, &session.id, &e1).unwrap();

        let scoped = events_in_session(&conn, &session.id, None).unwrap();
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].id, e1);
        assert_ne!(scoped[0].id, e2);

        // Attaching twice is idempotent.
        attach_event_to_session(&conn, &session.id, &e1).unwrap();
        let scoped = events_in_session(&conn, &session.id, None).unwrap();
        assert_eq!(scoped.len(), 1);
    }

    #[test]
    fn test_session_not_found_errors() {
        let (_file, conn) = test_db();
        let err = get_session(&conn, "no-such-session").unwrap_err();
        assert!(matches!(err, StateError::SessionNotFound(_)));
        let err = attach_event_to_session(&conn, "no-such-session", "e").unwrap_err();
        assert!(matches!(err, StateError::SessionNotFound(_)));
    }

    #[test]
    fn test_end_session_marks_ended_at() {
        let (_file, conn) = test_db();
        let session = create_session(&conn, "agent", None).unwrap();
        assert!(session.ended_at.is_none());

        let ended = end_session(&conn, &session.id).unwrap();
        assert!(ended.ended_at.is_some());

        // Ending again returns the existing session, does not error.
        let again = end_session(&conn, &session.id).unwrap();
        assert_eq!(again.ended_at, ended.ended_at);
    }

    #[test]
    fn test_provenance_is_append_only_lineage() {
        let (_file, conn) = test_db();
        let e1 = append(&conn, "cli", "baseline event", None).unwrap();

        let p1 = record_provenance(
            &conn,
            "baseline",
            "confidence_threshold",
            "initial value",
            "user",
            Some(&e1),
        )
        .unwrap();
        // Superseding = a new entry, old one remains.
        let p2 = record_provenance(
            &conn,
            "baseline",
            "confidence_threshold",
            "lowered after drift review",
            "agent",
            Some(&e1),
        )
        .unwrap();

        assert_ne!(p1.id, p2.id);
        let lineage = provenance_for(&conn, "baseline", "confidence_threshold").unwrap();
        assert_eq!(lineage.len(), 2);
        assert_eq!(lineage[0].id, p1.id);
        assert_eq!(lineage[1].id, p2.id);
    }

    #[test]
    fn test_build_context_includes_session_events_and_doubt() {
        let (_file, conn) = test_db();
        let session = create_session(&conn, "agent", None).unwrap();
        let old = append(&conn, "cli", "old event", None).unwrap();
        attach_event_to_session(&conn, &session.id, &old).unwrap();
        let new = append(&conn, "cli", "new event", None).unwrap();
        record_provenance(&conn, "event", &new, "derived summary", "agent", None).unwrap();

        let ctx = build_context(&conn, Some(&session.id), 1).unwrap();
        // limit=1 recent + 1 session-only event
        assert_eq!(ctx.events.len(), 2);
        assert!(ctx.session.is_some());
        assert_eq!(ctx.provenance.len(), 1);
        assert!(!ctx.doubt.might_have_missed.is_empty());
        assert!(!ctx.doubt.assumptions.is_empty());
        assert!(!ctx.doubt.might_break.is_empty());
    }

    #[test]
    fn test_build_context_without_session() {
        let (_file, conn) = test_db();
        append(&conn, "cli", "only event", None).unwrap();
        let ctx = build_context(&conn, None, 10).unwrap();
        assert_eq!(ctx.events.len(), 1);
        assert!(ctx.session.is_none());
        assert!(ctx.provenance.is_empty());
    }
}
