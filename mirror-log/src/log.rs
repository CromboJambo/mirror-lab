use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, Error as SqlError, Result, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const BUSY_RETRY_ATTEMPTS: usize = 10;
const BUSY_RETRY_DELAY_MS: u64 = 10;

#[derive(Debug, Clone, Copy)]
pub struct IntegrityReport {
    pub total_events: i64,
    pub missing_or_invalid_hashes: i64,
    pub hash_mismatches: i64,
    pub orphan_chunks: i64,
}

#[derive(Debug, Clone)]
pub struct AppendReceipt {
    pub id: String,
    pub timestamp: i64,
    pub ingested_at: i64,
    pub content_hash: String,
}

fn unix_now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn next_timestamp(conn: &Connection) -> Result<i64> {
    let now = unix_now_secs();
    let latest = conn.query_row("SELECT MAX(timestamp) FROM events", [], |row| {
        row.get::<_, Option<i64>>(0)
    })?;

    Ok(match latest {
        Some(last) if now <= last => last + 1,
        _ => now,
    })
}

pub fn append(
    conn: &Connection,
    source: &str,
    content: &str,
    meta: Option<&str>,
) -> Result<String> {
    append_with_receipt(conn, source, content, meta).map(|r| r.id)
}

pub fn append_with_receipt(
    conn: &Connection,
    source: &str,
    content: &str,
    meta: Option<&str>,
) -> Result<AppendReceipt> {
    with_busy_retry(|| {
        let tx = conn.unchecked_transaction()?;
        let receipt =
            append_single_event_internal(conn, source, content, meta, next_timestamp(conn)?)?;
        tx.commit()?;
        Ok(receipt)
    })
}

pub fn append_batch(
    conn: &Connection,
    source: &str,
    contents: &[&str],
    meta: Option<&str>,
) -> Result<Vec<String>> {
    append_batch_with_receipts(conn, source, contents, meta)
        .map(|r| r.into_iter().map(|rec| rec.id).collect())
}

pub fn append_batch_with_receipts(
    conn: &Connection,
    source: &str,
    contents: &[&str],
    meta: Option<&str>,
) -> Result<Vec<AppendReceipt>> {
    with_busy_retry(|| {
        let tx = conn.unchecked_transaction()?;
        let receipts = append_batch_with_receipts_in_tx(&tx, source, contents, meta)?;
        tx.commit()?;
        Ok(receipts)
    })
}

pub fn append_batch_with_receipts_in_tx(
    conn: &Connection,
    source: &str,
    contents: &[&str],
    meta: Option<&str>,
) -> Result<Vec<AppendReceipt>> {
    let mut receipts = Vec::with_capacity(contents.len());
    let mut timestamp = next_timestamp(conn)?;

    for content in contents {
        receipts.push(append_single_event_internal(
            conn, source, content, meta, timestamp,
        )?);
        timestamp += 1;
    }

    Ok(receipts)
}

fn append_single_event_internal(
    conn: &Connection,
    source: &str,
    content: &str,
    meta: Option<&str>,
    timestamp: i64,
) -> Result<AppendReceipt> {
    let id = Uuid::new_v4().to_string();
    let ingested_at = timestamp;

    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());

    conn.execute(
        "INSERT INTO events (id, timestamp, source, content, meta, ingested_at, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            timestamp,
            source,
            content,
            meta,
            ingested_at,
            content_hash
        ],
    )?;

    Ok(AppendReceipt {
        id,
        timestamp,
        ingested_at,
        content_hash,
    })
}

pub fn append_stdin(
    conn: &Connection,
    source: &str,
    meta: Option<&str>,
    batch_size: usize,
) -> Result<Vec<String>> {
    let mut batch = Vec::with_capacity(batch_size);
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();

    while batch.len() < batch_size
        && std::io::BufRead::read_line(&mut reader, &mut line).unwrap_or(0) > 0
    {
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            batch.push(trimmed);
        }
        line.clear();
    }

    if batch.is_empty() {
        return Ok(vec![]);
    }

    append_batch(
        conn,
        source,
        &batch.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        meta,
    )
}

pub fn verify_integrity(conn: &Connection) -> Result<IntegrityReport> {
    let total_events = conn.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;

    let mut stmt = conn
        .prepare("SELECT id, content, content_hash FROM events WHERE content_hash IS NOT NULL")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    let mut missing_or_invalid_hashes: i64 = 0;
    let mut hash_mismatches: i64 = 0;

    for row_result in rows {
        let (_id, content, stored_hash) = row_result?;

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let computed_hash = format!("{:x}", hasher.finalize());

        if computed_hash != stored_hash {
            hash_mismatches += 1;
        }
    }

    // Count events with NULL content_hash
    let null_hashes: i64 = conn.query_row(
        "SELECT COUNT(*) FROM events WHERE content_hash IS NULL",
        [],
        |row| row.get(0),
    )?;
    missing_or_invalid_hashes += null_hashes;

    // Count orphan chunks (chunks referencing non-existent events)
    let orphan_chunks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM chunks WHERE event_id NOT IN (SELECT id FROM events)",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(IntegrityReport {
        total_events,
        missing_or_invalid_hashes,
        hash_mismatches,
        orphan_chunks,
    })
}

pub fn stats(conn: &Connection) -> Result<(i64, i64, i64, i64)> {
    let total = conn.query_row("SELECT COUNT(*) FROM active_events", [], |row| row.get(0))?;

    let unique = conn.query_row(
        "SELECT COUNT(DISTINCT content_hash) FROM active_events",
        [],
        |row| row.get(0),
    )?;

    let oldest = conn
        .query_row("SELECT MIN(timestamp) FROM active_events", [], |row| {
            row.get::<_, Option<i64>>(0)
        })
        .map(|opt| opt.unwrap_or(0))?;

    let newest = conn
        .query_row("SELECT MAX(timestamp) FROM active_events", [], |row| {
            row.get::<_, Option<i64>>(0)
        })
        .map(|opt| opt.unwrap_or(0))?;

    Ok((total, unique, oldest, newest))
}

fn with_busy_retry<T, F>(mut operation: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    let mut attempts = 0;
    loop {
        match operation() {
            Ok(value) => return Ok(value),
            Err(err) if is_busy_error(&err) && attempts < BUSY_RETRY_ATTEMPTS => {
                attempts += 1;
                thread::sleep(std::time::Duration::from_millis(BUSY_RETRY_DELAY_MS));
            }
            Err(err) => return Err(err),
        }
    }
}

fn is_busy_error(err: &SqlError) -> bool {
    matches!(
        err,
        SqlError::SqliteFailure(code, _)
            if matches!(
                code.code,
                rusqlite::ffi::ErrorCode::DatabaseBusy | rusqlite::ffi::ErrorCode::DatabaseLocked
            )
    )
}

pub fn is_duplicate(conn: &Connection, content_hash: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM events WHERE content_hash = ?1)",
        params![content_hash],
        |row| row.get(0),
    )
}
