use mirror_log::{AppendReceipt, append_batch_with_receipts, verify_integrity};
use rusqlite::{Connection, Result};
use std::fs;
use std::path::Path;

#[test]
fn test_threshold_implementation() -> Result<()> {
    // 1. Setup: Create an in-memory database for testing
    let conn = Connection::open_in_memory()?;

    // Initialize schema (Simplified version of schema.sql for the test)
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

    // Ensure staging directory exists for the test
    let staging_dir = Path::new("mirror-log/staging");
    if staging_dir.exists() {
        fs::remove_dir_all(staging_dir).unwrap();
    }
    fs::create_dir_all(staging_dir).unwrap();

    // 2. Test Case A: Small Payload (The Sense)
    let small_content = "This is a small, fast event.";
    let receipt_small =
        append_batch_with_receipts(&conn, "test_src", &[small_content], None)?[0].clone();

    let mut stmt = conn.prepare("SELECT content FROM events WHERE id = ?1")?;
    let content: String = stmt.query_row([&receipt_small.id], |row| row.get(0))?;

    assert_eq!(content, small_content);
    println!("✅ Small payload handled correctly in 'events' table.");

    // 3. Test Case B: Large Payload (The Experience)
    // Create a payload larger than the 64KB threshold
    let large_content = "A".repeat(70000);
    let receipts_large =
        append_batch_with_receipts(&conn, "test_src", &[&large_content], Some("heavy_metadata"))?;
    let receipt_large = receipts_large[0].clone();

    // Verify 'events' table contains the stub
    let event_stub: String = conn.query_row(
        "SELECT content FROM events WHERE id = ?1",
        [&receipt_large.id],
        |row| row.get(0),
    )?;
    assert!(event_stub.contains("[CHUNK:"));
    println!("✅ Large payload correctly stubbed in 'events' table.");

    // Verify 'chunks' table contains the actual content
    let chunk_content: String = conn.query_row(
        "SELECT content FROM chunks WHERE event_id = ?1",
        [&receipt_large.id],
        |row| row.get(0),
    )?;
    assert_eq!(chunk_content, large_content);
    println!("✅ Large payload content preserved in 'chunks' table.");

    // 4. Verify Integrity
    let report = verify_integrity(&conn)?;
    assert_eq!(report.total_events, 2);
    assert_eq!(report.hash_mismatches, 0);
    println!(
        "✅ Integrity check passed: Total events = {}",
        report.total_events
    );

    // 5. Cleanup (Optional in tests, but good practice)
    // Note: In a real CI environment, we'd use a temporary directory for staging.

    Ok(())
}
