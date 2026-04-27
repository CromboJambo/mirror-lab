// src/db.rs
use anyhow::Result;
use chrono::{DateTime, Timelike, Utc};
use rusqlite::{Connection, Error as SqlError, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single chunk of processed activity.
/// This is the atom of the storage layer — everything downstream
/// (distillation, gap detection, retention) operates on chunks.
#[derive(Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub id: Option<i64>,
    pub source_file: String,          // original OBS recording filename
    pub chunk_index: u32,             // position within the source recording
    pub chunk_path: String,           // path to the chunk video file on disk
    pub started_at: DateTime<Utc>,    // wall clock time this chunk begins
    pub duration_secs: f64,           // actual duration after auto-editor removal
    pub raw_duration_secs: f64,       // duration before auto-editor (gap reference)
    pub ocr_text: Option<String>,     // extracted screen text (null until OCR runs)
    pub transcript: Option<String>,   // audio transcript (null until whisper runs)
    pub window_title: Option<String>, // active window if detectable
    pub source_type: String,          // filesystem, clipboard, etc.
    pub importance_score: f64,        // heuristic score for distillation priority
    pub metadata: Option<String>,     // JSON landing zone for unpromoted properties
    pub distillation_tier: u8,        // 0=fine, 1=first distill, 2=second distill
    pub retained: bool,               // user explicitly kept this at distillation
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkGroupSummary {
    pub group_id: i64,
    pub group_type: String,
    pub group_key: String,
    pub label: String,
    pub chunk_count: i64,
    pub first_started_at: Option<DateTime<Utc>>,
    pub last_started_at: Option<DateTime<Utc>>,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let db = Db { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;

            CREATE TABLE IF NOT EXISTS chunks (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                source_file         TEXT NOT NULL,
                chunk_index         INTEGER NOT NULL,
                chunk_path          TEXT NOT NULL UNIQUE,
                started_at          TEXT NOT NULL,
                duration_secs       REAL NOT NULL,
                raw_duration_secs   REAL NOT NULL,
                ocr_text            TEXT,
                transcript          TEXT,
                window_title        TEXT,
                metadata            TEXT,
                source_type         TEXT NOT NULL,
                importance_score    REAL NOT NULL DEFAULT 0.0,
                distillation_tier   INTEGER NOT NULL DEFAULT 0,
                retained            INTEGER NOT NULL DEFAULT 0,
                created_at          TEXT NOT NULL
            );

            -- Full text search over ocr + transcript
            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts
            USING fts5(
                ocr_text,
                transcript,
                content='chunks',
                content_rowid='id'
            );

            -- Trigger to keep FTS in sync
            CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
                INSERT INTO chunks_fts(rowid, ocr_text, transcript)
                VALUES (new.id, new.ocr_text, new.transcript);
            END;

            CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
                INSERT INTO chunks_fts(chunks_fts, rowid, ocr_text, transcript)
                VALUES ('delete', old.id, old.ocr_text, old.transcript);
                INSERT INTO chunks_fts(rowid, ocr_text, transcript)
                VALUES (new.id, new.ocr_text, new.transcript);
            END;

            -- Retention events: what the user decided at distillation time
            CREATE TABLE IF NOT EXISTS retention_events (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                chunk_id    INTEGER NOT NULL REFERENCES chunks(id),
                tier        INTEGER NOT NULL,
                decision    TEXT NOT NULL, -- 'keep' | 'release'
                decided_at  TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chunk_groups (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                group_type  TEXT NOT NULL,
                group_key   TEXT NOT NULL,
                label       TEXT NOT NULL,
                created_at  TEXT NOT NULL,
                UNIQUE(group_type, group_key)
            );

            CREATE TABLE IF NOT EXISTS chunk_group_members (
                group_id     INTEGER NOT NULL REFERENCES chunk_groups(id) ON DELETE CASCADE,
                chunk_id     INTEGER NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
                ordinal      INTEGER,
                joined_at    TEXT NOT NULL,
                PRIMARY KEY (group_id, chunk_id)
            );
        ",
        )?;
        Ok(())
    }

    pub fn insert_chunk(&self, chunk: &Chunk) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO chunks (
                source_file, chunk_index, chunk_path, started_at,
                duration_secs, raw_duration_secs, ocr_text, transcript,
                window_title, metadata, source_type, importance_score, distillation_tier, retained, created_at
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![
                chunk.source_file,
                chunk.chunk_index,
                chunk.chunk_path,
                chunk.started_at.to_rfc3339(),
                chunk.duration_secs,
                chunk.raw_duration_secs,
                chunk.ocr_text,
                chunk.transcript,
                chunk.window_title,
                chunk.metadata,
                chunk.source_type,
                chunk.importance_score,
                chunk.distillation_tier,
                if chunk.retained { 1 } else { 0 },
                chunk.created_at.to_rfc3339(),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn insert_or_get_chunk(&self, chunk: &Chunk) -> Result<(i64, bool)> {
        match self.insert_chunk(chunk) {
            Ok(id) => Ok((id, true)),
            Err(err) if is_unique_constraint(&err) => {
                let id = self.chunk_id_by_path(&chunk.chunk_path)?;
                Ok((id, false))
            }
            Err(err) => Err(err),
        }
    }

    pub fn chunk_id_by_path(&self, chunk_path: &str) -> Result<i64> {
        self.conn
            .query_row(
                "SELECT id FROM chunks WHERE chunk_path = ?1",
                params![chunk_path],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn update_transcript(&self, chunk_id: i64, transcript: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE chunks SET transcript = ?1 WHERE id = ?2",
            params![transcript, chunk_id],
        )?;
        Ok(())
    }

    pub fn transcript_missing(&self, chunk_id: i64) -> Result<bool> {
        let transcript: Option<String> = self.conn.query_row(
            "SELECT transcript FROM chunks WHERE id = ?1",
            params![chunk_id],
            |row| row.get(0),
        )?;
        Ok(transcript
            .as_deref()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true))
    }

    pub fn ensure_chunk_group(
        &self,
        group_type: &str,
        group_key: &str,
        label: &str,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO chunk_groups (group_type, group_key, label, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(group_type, group_key)
             DO UPDATE SET label = excluded.label",
            params![group_type, group_key, label, Utc::now().to_rfc3339()],
        )?;

        self.conn
            .query_row(
                "SELECT id FROM chunk_groups WHERE group_type = ?1 AND group_key = ?2",
                params![group_type, group_key],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn add_chunk_to_group(
        &self,
        group_id: i64,
        chunk_id: i64,
        ordinal: Option<u32>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO chunk_group_members (group_id, chunk_id, ordinal, joined_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(group_id, chunk_id)
             DO UPDATE SET ordinal = excluded.ordinal",
            params![
                group_id,
                chunk_id,
                ordinal.map(i64::from),
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn ensure_recording_group(&self, source_file: &str) -> Result<i64> {
        self.ensure_chunk_group("recording", source_file, source_file)
    }

    pub fn ensure_hour_group(&self, started_at: DateTime<Utc>) -> Result<i64> {
        let bucket = started_at
            .with_minute(0)
            .and_then(|dt| dt.with_second(0))
            .and_then(|dt| dt.with_nanosecond(0))
            .unwrap_or(started_at);
        let group_key = bucket.to_rfc3339();
        let label = bucket.format("%Y-%m-%d %H:00 UTC").to_string();
        self.ensure_chunk_group("hour", &group_key, &label)
    }

    pub fn ensure_window_group(&self, window_title: &str) -> Result<i64> {
        self.ensure_chunk_group("window", window_title, window_title)
    }

    pub fn group_summary(
        &self,
        group_type: &str,
        group_key: &str,
    ) -> Result<Option<ChunkGroupSummary>> {
        self.conn
            .query_row(
                "SELECT
                    g.id,
                    g.group_type,
                    g.group_key,
                    g.label,
                    COUNT(m.chunk_id) as chunk_count,
                    MIN(c.started_at) as first_started_at,
                    MAX(c.started_at) as last_started_at
                 FROM chunk_groups g
                 LEFT JOIN chunk_group_members m ON m.group_id = g.id
                 LEFT JOIN chunks c ON c.id = m.chunk_id
                 WHERE g.group_type = ?1 AND g.group_key = ?2
                 GROUP BY g.id, g.group_type, g.group_key, g.label",
                params![group_type, group_key],
                |row| {
                    Ok(ChunkGroupSummary {
                        group_id: row.get(0)?,
                        group_type: row.get(1)?,
                        group_key: row.get(2)?,
                        label: row.get(3)?,
                        chunk_count: row.get(4)?,
                        first_started_at: parse_optional_rfc3339(row.get(5)?)?,
                        last_started_at: parse_optional_rfc3339(row.get(6)?)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn recording_group_summary(&self, source_file: &str) -> Result<Option<ChunkGroupSummary>> {
        self.group_summary("recording", source_file)
    }

    pub fn hour_group_summary(
        &self,
        started_at: DateTime<Utc>,
    ) -> Result<Option<ChunkGroupSummary>> {
        let bucket = started_at
            .with_minute(0)
            .and_then(|dt| dt.with_second(0))
            .and_then(|dt| dt.with_nanosecond(0))
            .unwrap_or(started_at);
        self.group_summary("hour", &bucket.to_rfc3339())
    }

    pub fn window_group_summary(&self, window_title: &str) -> Result<Option<ChunkGroupSummary>> {
        self.group_summary("window", window_title)
    }

    /// Chunks due for first distillation review
    pub fn chunks_due_for_distillation(&self, fine_grain_days: u64) -> Result<Vec<Chunk>> {
        let cutoff = Utc::now() - chrono::Duration::days(fine_grain_days as i64);
        let mut stmt = self.conn.prepare(
            "SELECT * FROM chunks
             WHERE distillation_tier = 0
             AND created_at < ?1
             AND retained = 0
             ORDER BY started_at ASC",
        )?;
        let chunks = stmt
            .query_map(params![cutoff.to_rfc3339()], row_to_chunk)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(chunks)
    }

    /// Mark a chunk as released — will be cleaned up on next gc pass
    #[allow(dead_code)]
    pub fn release_chunk(&self, chunk_id: i64, tier: u8) -> Result<()> {
        self.conn.execute(
            "UPDATE chunks SET distillation_tier = ?1 WHERE id = ?2",
            params![tier, chunk_id],
        )?;
        self.conn.execute(
            "INSERT INTO retention_events (chunk_id, tier, decision, decided_at)
             VALUES (?1, ?2, 'release', ?3)",
            params![chunk_id, tier, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Mark a chunk as explicitly retained by the user
    #[allow(dead_code)]
    pub fn retain_chunk(&self, chunk_id: i64, tier: u8) -> Result<()> {
        self.conn.execute(
            "UPDATE chunks SET retained = 1, distillation_tier = ?1 WHERE id = ?2",
            params![tier, chunk_id],
        )?;
        self.conn.execute(
            "INSERT INTO retention_events (chunk_id, tier, decision, decided_at)
             VALUES (?1, ?2, 'keep', ?3)",
            params![chunk_id, tier, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Oversaturation: topics/windows appearing more than threshold across chunks
    /// Returns (window_title_or_text_fragment, count) sorted descending
    pub fn oversaturation_report(&self, min_count: u32) -> Result<Vec<(String, u32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT window_title, COUNT(*) as cnt
             FROM chunks
             WHERE window_title IS NOT NULL
             AND distillation_tier = 0
             GROUP BY window_title
             HAVING cnt >= ?1
             ORDER BY cnt DESC",
        )?;
        let rows = stmt
            .query_map(params![min_count], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

fn is_unique_constraint(err: &anyhow::Error) -> bool {
    err.downcast_ref::<SqlError>().is_some_and(|sql_err| {
        matches!(
            sql_err,
            SqlError::SqliteFailure(code, _)
                if code.code == rusqlite::ffi::ErrorCode::ConstraintViolation
        )
    })
}

fn row_to_chunk(row: &rusqlite::Row) -> rusqlite::Result<Chunk> {
    Ok(Chunk {
        id: Some(row.get(0)?),
        source_file: row.get(1)?,
        chunk_index: row.get(2)?,
        chunk_path: row.get(3)?,
        started_at: row
            .get::<_, String>(4)?
            .parse::<DateTime<Utc>>()
            .unwrap_or(Utc::now()),
        duration_secs: row.get(5)?,
        raw_duration_secs: row.get(6)?,
        ocr_text: row.get(7)?,
        transcript: row.get(8)?,
        window_title: row.get(9)?,
        metadata: row.get::<_, Option<String>>(10)?,
        source_type: row.get::<_, String>(11)?,
        importance_score: row.get::<_, f64>(12)?,
        distillation_tier: row.get::<_, u8>(13)?,
        retained: row.get::<_, i32>(14)? != 0,
        created_at: row
            .get::<_, String>(15)?
            .parse::<DateTime<Utc>>()
            .unwrap_or(Utc::now()),
    })
}

fn parse_optional_rfc3339(value: Option<String>) -> rusqlite::Result<Option<DateTime<Utc>>> {
    match value {
        Some(raw) => raw.parse::<DateTime<Utc>>().map(Some).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        }),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db_path(test_name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "ingress_{}_{}_{}.db",
            test_name,
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        p
    }

    #[test]
    fn update_transcript_persists_to_chunks_and_fts() {
        let db_path = temp_db_path("update_transcript");
        let db = Db::open(&db_path).expect("db open");

        let chunk = Chunk {
            id: None,
            source_file: "source.mp4".to_string(),
            chunk_index: 0,
            chunk_path: "/tmp/chunk_0000.mkv".to_string(),
            started_at: Utc::now(),
            duration_secs: 10.0,
            raw_duration_secs: 12.0,
            ocr_text: None,
            transcript: None,
            window_title: None,
            source_type: "filesystem".to_string(),
            importance_score: 1.0,
            metadata: None,
            distillation_tier: 0,
            retained: false,
            created_at: Utc::now(),
        };

        let id = db.insert_chunk(&chunk).expect("insert chunk");
        db.update_transcript(id, "hello whisper")
            .expect("update transcript");

        let transcript: Option<String> = db
            .conn
            .query_row(
                "SELECT transcript FROM chunks WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .expect("select transcript");
        assert_eq!(transcript.as_deref(), Some("hello whisper"));

        let fts_hits: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM chunks_fts WHERE chunks_fts MATCH 'whisper'",
                [],
                |row| row.get(0),
            )
            .expect("fts query");
        assert_eq!(fts_hits, 1);

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn insert_or_get_chunk_reuses_existing_row_for_duplicate_path() {
        let db_path = temp_db_path("insert_or_get_chunk");
        let db = Db::open(&db_path).expect("db open");

        let chunk = Chunk {
            id: None,
            source_file: "source.mp4".to_string(),
            chunk_index: 0,
            chunk_path: "/tmp/chunk_duplicate.mkv".to_string(),
            started_at: Utc::now(),
            duration_secs: 10.0,
            raw_duration_secs: 12.0,
            ocr_text: None,
            transcript: Some(String::new()),
            window_title: None,
            source_type: "filesystem".to_string(),
            importance_score: 1.0,
            metadata: None,
            distillation_tier: 0,
            retained: false,
            created_at: Utc::now(),
        };

        let (first_id, inserted) = db.insert_or_get_chunk(&chunk).expect("first insert");
        assert!(inserted);

        let (second_id, inserted) = db.insert_or_get_chunk(&chunk).expect("duplicate insert");
        assert!(!inserted);
        assert_eq!(first_id, second_id);

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn recording_group_summary_tracks_chunk_membership() {
        let db_path = temp_db_path("recording_group_summary");
        let db = Db::open(&db_path).expect("db open");
        let group_id = db
            .ensure_recording_group("session_001.mp4")
            .expect("ensure group");

        for idx in 0..2 {
            let chunk = Chunk {
                id: None,
                source_file: "session_001.mp4".to_string(),
                chunk_index: idx,
                chunk_path: format!("/tmp/session_001_chunk_{idx:04}.mkv"),
                started_at: Utc::now() + chrono::Duration::seconds(i64::from(idx) * 30),
                duration_secs: 10.0,
                raw_duration_secs: 12.0,
                ocr_text: None,
                transcript: Some(String::new()),
                window_title: None,
                source_type: "filesystem".to_string(),
                importance_score: 1.0,
                metadata: None,
                distillation_tier: 0,
                retained: false,
                created_at: Utc::now(),
            };

            let (chunk_id, _) = db.insert_or_get_chunk(&chunk).expect("insert chunk");
            db.add_chunk_to_group(group_id, chunk_id, Some(idx))
                .expect("attach chunk");
        }

        let summary = db
            .recording_group_summary("session_001.mp4")
            .expect("summary query")
            .expect("summary exists");
        assert_eq!(summary.group_type, "recording");
        assert_eq!(summary.group_key, "session_001.mp4");
        assert_eq!(summary.chunk_count, 2);
        assert!(summary.first_started_at.is_some());
        assert!(summary.last_started_at.is_some());

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn hour_group_summary_tracks_chunks_across_recordings_in_same_bucket() {
        let db_path = temp_db_path("hour_group_summary");
        let db = Db::open(&db_path).expect("db open");
        let bucket_time = DateTime::parse_from_rfc3339("2026-04-16T14:23:00Z")
            .expect("parse time")
            .with_timezone(&Utc);
        let hour_group_id = db
            .ensure_hour_group(bucket_time)
            .expect("ensure hour group");

        for (idx, source_file) in ["session_a.mp4", "session_b.mp4"].into_iter().enumerate() {
            let chunk = Chunk {
                id: None,
                source_file: source_file.to_string(),
                chunk_index: idx as u32,
                chunk_path: format!("/tmp/hour_bucket_chunk_{idx:04}.mkv"),
                started_at: bucket_time + chrono::Duration::minutes((idx as i64) * 10),
                duration_secs: 10.0,
                raw_duration_secs: 12.0,
                ocr_text: None,
                transcript: Some(String::new()),
                window_title: None,
                source_type: "filesystem".to_string(),
                importance_score: 1.0,
                metadata: None,
                distillation_tier: 0,
                retained: false,
                created_at: Utc::now(),
            };

            let (chunk_id, _) = db.insert_or_get_chunk(&chunk).expect("insert chunk");
            db.add_chunk_to_group(hour_group_id, chunk_id, Some(chunk.chunk_index))
                .expect("attach hour chunk");
        }

        let summary = db
            .hour_group_summary(bucket_time)
            .expect("hour summary query")
            .expect("hour summary exists");
        assert_eq!(summary.group_type, "hour");
        assert_eq!(summary.chunk_count, 2);
        assert_eq!(summary.label, "2026-04-16 14:00 UTC");

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn window_group_summary_tracks_chunks_across_sessions() {
        let db_path = temp_db_path("window_group_summary");
        let db = Db::open(&db_path).expect("db open");
        let window_title = "Neovim - mirror-lab";
        let window_group_id = db
            .ensure_window_group(window_title)
            .expect("ensure window group");

        for (idx, source_file) in ["session_a.mp4", "session_b.mp4"].into_iter().enumerate() {
            let chunk = Chunk {
                id: None,
                source_file: source_file.to_string(),
                chunk_index: idx as u32,
                chunk_path: format!("/tmp/window_bucket_chunk_{idx:04}.mkv"),
                started_at: Utc::now() + chrono::Duration::minutes((idx as i64) * 5),
                duration_secs: 10.0,
                raw_duration_secs: 12.0,
                ocr_text: None,
                transcript: Some(String::new()),
                window_title: Some(window_title.to_string()),
                source_type: "filesystem".to_string(),
                importance_score: 1.0,
                metadata: None,
                distillation_tier: 0,
                retained: false,
                created_at: Utc::now(),
            };

            let (chunk_id, _) = db.insert_or_get_chunk(&chunk).expect("insert chunk");
            db.add_chunk_to_group(window_group_id, chunk_id, Some(chunk.chunk_index))
                .expect("attach window chunk");
        }

        let summary = db
            .window_group_summary(window_title)
            .expect("window summary query")
            .expect("window summary exists");
        assert_eq!(summary.group_type, "window");
        assert_eq!(summary.group_key, window_title);
        assert_eq!(summary.chunk_count, 2);

        let _ = std::fs::remove_file(db_path);
    }
}
