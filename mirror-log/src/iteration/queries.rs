//! Iteration tracking query functions

use rusqlite::{Connection, Result};

/// Get all iteration passes for a specific event
pub fn get_iteration_passes(conn: &Connection, event_id: &str) -> Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT id FROM iteration_passes WHERE event_id = ?1 ORDER BY created_at")?;
    let rows = stmt.query_map([event_id], |row| row.get(0))?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Get iteration status for an event
pub fn get_iteration_status(conn: &Connection, event_id: &str) -> Result<Option<String>> {
    let mut stmt =
        conn.prepare("SELECT current_pass_type FROM iteration_status WHERE event_id = ?1")?;

    match stmt.query_row([event_id], |row| row.get(0)) {
        Ok(status) => Ok(Some(status)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Insert a new iteration pass
pub fn insert_iteration_pass(
    conn: &Connection,
    event_id: &str,
    iteration_number: i32,
    pass_type: &str,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO iteration_passes (id, event_id, iteration_number, pass_type, created_at) VALUES (?1, ?2, ?3, ?4, unixepoch())"
    )?;

    stmt.execute([
        uuid::Uuid::new_v4().to_string(),
        event_id.to_string(),
        iteration_number.to_string(),
        pass_type.to_string(),
    ])?;
    Ok(())
}

/// Update iteration status
pub fn update_iteration_status(
    conn: &Connection,
    event_id: &str,
    current_iteration: i32,
    current_pass_type: Option<&str>,
    is_complete: bool,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO iteration_status (id, event_id, current_iteration, current_pass_type, is_complete, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, unixepoch())"
    )?;

    stmt.execute([
        uuid::Uuid::new_v4().to_string(),
        event_id.to_string(),
        current_iteration.to_string(),
        current_pass_type.unwrap_or("").to_string(),
        (is_complete as i32).to_string(),
    ])?;
    Ok(())
}
