use mirror_log::{append_batch_with_receipts, verify_integrity};
use rusqlite::{Connection, Result};

#[test]
fn test_threshold_implementation() -> Result<()> {
    let conn = Connection::open_in_memory()?;

    conn.execute_batch(
        "CREATE TABLE events (
            id TEXT PRIMARY KEY,
            timestamp INTEGER NOT NULL,
            source TEXT NOT NULL,
            content TEXT NOT NULL,
            meta TEXT,
            ingested_at INTEGER NOT NULL,
            content_hash TEXT
        );
        CREATE TABLE chunks (
            id TEXT PRIMARY KEY,
            event_id TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            content TEXT NOT NULL,
            start_offset INTEGER NOT NULL,
            end_offset INTEGER NOT NULL,
            timestamp INTEGER NOT NULL,
            FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
        );
        CREATE TABLE shadow_state (
            event_id TEXT PRIMARY KEY,
            at INTEGER NOT NULL
        );",
    )?;

    // Small payload: persisted in full, no chunks created
    let small_content = "This is a small, fast event.";
    let receipt_small =
        append_batch_with_receipts(&conn, "test_src", &[small_content], None)?[0].clone();

    let mut stmt = conn.prepare("SELECT content FROM events WHERE id = ?1")?;
    let content: String = stmt.query_row([&receipt_small.id], |row| row.get(0))?;

    assert_eq!(content, small_content);
    println!("✅ Small payload handled correctly in 'events' table.");

    // Large payload: persisted in full AND chunked additively
    let large_content = "A".repeat(70000);
    let receipts_large =
        append_batch_with_receipts(&conn, "test_src", &[&large_content], Some("heavy_metadata"))?;
    let receipt_large = receipts_large[0].clone();

    // Verify 'events' table still contains the full content (additive model, not stub-based)
    let event_content: String = conn.query_row(
        "SELECT content FROM events WHERE id = ?1",
        [&receipt_large.id],
        |row| row.get(0),
    )?;
    assert_eq!(event_content, large_content);
    println!("✅ Large payload persisted in full in 'events' table (additive model).");

    // Chunking is additive: chunks are created separately via the pipeline path,
    // not automatically by append_batch_with_receipts.
    // Verify no chunks were auto-created by the append path.
    let chunk_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM chunks WHERE event_id = ?1",
        [&receipt_large.id],
        |row| row.get(0),
    )?;
    assert_eq!(chunk_count, 0);
    println!("✅ No auto-chunks created by append path (chunking is pipeline-gated).");

    // Verify Integrity
    let report = verify_integrity(&conn)?;
    assert_eq!(report.total_events, 2);
    assert_eq!(report.hash_mismatches, 0);
    println!(
        "✅ Integrity check passed: Total events = {}",
        report.total_events
    );

    Ok(())
}
