use rusqlite::{Connection, Result};
use uuid::Uuid;

#[derive(Debug)]
pub struct Chunk {
    pub id: String,
    pub event_id: String,
    pub chunk_index: i64,
    pub content: String,
    pub start_offset: i64, //placeholder for now
    pub end_offset: i64,   // placeholder for now
}

/// Split content into chunks based on paragraphs or size
pub fn chunk_content(content: &str, max_chunk_size: usize) -> Vec<(usize, usize, String)> {
    let mut chunks = Vec::new();

    if content.is_empty() || max_chunk_size == 0 {
        return chunks;
    }

    let mut start = 0;
    while start < content.len() {
        let remaining = content.len() - start;
        if remaining <= max_chunk_size {
            chunks.push((start, content.len(), content[start..].to_string()));
            break;
        }

        let target_end = max_chunk_size.min(remaining);
        let mut hard_end = start;
        for (offset, _) in content[start..].char_indices() {
            if offset <= target_end {
                hard_end = start + offset;
            } else {
                break;
            }
        }
        if hard_end == start
            && let Some((offset, ch)) = content[start..].char_indices().next()
        {
            hard_end = start + offset + ch.len_utf8();
        }
        let window = &content[start..hard_end];

        // Prefer splitting on whitespace for readability; fall back to a hard boundary.
        let split_at = window
            .char_indices()
            .filter_map(|(idx, ch)| {
                if idx > 0 && ch.is_whitespace() {
                    Some(start + idx)
                } else {
                    None
                }
            })
            .next_back()
            .unwrap_or(hard_end);

        chunks.push((start, split_at, content[start..split_at].to_string()));
        start = split_at;
    }

    chunks
}

/// Create chunks for an event
pub fn create_chunks(
    conn: &Connection,
    event_id: &str,
    content: &str,
    timestamp: i64,
    max_chunk_size: usize,
) -> Result<usize> {
    let chunks = chunk_content(content, max_chunk_size);

    let mut count = 0;
    for (idx, (start, end, chunk_content)) in chunks.iter().enumerate() {
        let chunk_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO chunks (id, event_id, chunk_index, content, start_offset, end_offset, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                &chunk_id,
                event_id,
                idx as i64,
                chunk_content,
                *start as i64,
                *end as i64,
                timestamp,
            ),
        )?;
        count += 1;
    }

    Ok(count)
}

/// Search chunks
pub fn search_chunks(conn: &Connection, term: &str, limit: Option<i64>) -> Result<Vec<Chunk>> {
    let like = format!("%{}%", term);

    let query = if let Some(lim) = limit {
        format!(
            "SELECT id, event_id, chunk_index, content, start_offset, end_offset
             FROM chunks
             WHERE content LIKE ?1
             AND NOT EXISTS (
                 SELECT 1 FROM shadow_state s WHERE s.event_id = chunks.event_id
             )
             ORDER BY timestamp DESC
             LIMIT {}",
            lim
        )
    } else {
        "SELECT id, event_id, chunk_index, content, start_offset, end_offset
         FROM chunks
         WHERE content LIKE ?1
         AND NOT EXISTS (
             SELECT 1 FROM shadow_state s WHERE s.event_id = chunks.event_id
         )
         ORDER BY timestamp DESC"
            .to_string()
    };

    let mut stmt = conn.prepare(&query)?;

    let rows = stmt.query_map([like], |row| {
        Ok(Chunk {
            id: row.get(0)?,
            event_id: row.get(1)?,
            chunk_index: row.get(2)?,
            content: row.get(3)?,
            start_offset: row.get(4)?,
            end_offset: row.get(5)?,
        })
    })?;

    rows.collect()
}

/// List all chunks for an event
pub fn list_chunks(conn: &Connection, event_id: &str) -> Result<Vec<Chunk>> {
    let mut stmt = conn.prepare(
        "SELECT id, event_id, chunk_index, content, start_offset, end_offset
         FROM chunks
         WHERE event_id = ?1
         AND NOT EXISTS (
             SELECT 1 FROM shadow_state s WHERE s.event_id = chunks.event_id
         )
         ORDER BY chunk_index ASC",
    )?;

    let rows = stmt.query_map([event_id], |row| {
        Ok(Chunk {
            id: row.get(0)?,
            event_id: row.get(1)?,
            chunk_index: row.get(2)?,
            content: row.get(3)?,
            start_offset: row.get(4)?,
            end_offset: row.get(5)?,
        })
    })?;

    rows.collect()
}
