use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn temp_db() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push("mirror_log_test_");
    let random: u64 = rand::random();
    path.push(format!("test_{}.db", random));

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }

    path
}

fn get_binary_path() -> PathBuf {
    std::env::var("CARGO_BIN_EXE_mirror_log")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_mirror-log"))
        .map(PathBuf::from)
        .or_else(|_| {
            let mut path = std::env::current_exe().map_err(|_| std::env::VarError::NotPresent)?;
            path.pop();
            path.pop();
            path.push(format!("mirror-log{}", std::env::consts::EXE_SUFFIX));
            Ok::<PathBuf, std::env::VarError>(path)
        })
        .expect("Cargo binary path environment variable not set")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_basic_append() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let result = mirror_log::log::append(&conn, "test_source", "Test event content", None)
            .expect("Failed to append");

        assert!(!result.is_empty());
        assert_eq!(result.len(), 36); // UUID length

        // Verify it was actually stored
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);
        assert_eq!(_unique, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_duplicate_detection() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let content = "This is duplicate content";

        // First append
        mirror_log::log::append(&conn, "source1", content, None).expect("Failed to append first");

        // Second append with same content
        mirror_log::log::append(&conn, "source2", content, None).expect("Failed to append second");

        // Verify duplicate detection
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 2);
        assert_eq!(_unique, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_batch_append() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let contents = vec!["Batch event 1", "Batch event 2", "Batch event 3"];

        let result = mirror_log::log::append_batch(&conn, "batch_source", &contents, None)
            .expect("Failed to batch append");

        assert_eq!(result.len(), 3);

        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 3);
        assert_eq!(_unique, 3);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_stdin_append() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Write to stdin
        let input = "Stdin event 1\nStdin event 2\nStdin event 3\n";
        let mut child = Command::new(get_binary_path())
            .args([
                "--db",
                db_path.to_str().unwrap(),
                "stdin",
                "--source",
                "stdin_test",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input.as_bytes())
                .expect("Failed to write to stdin");
        }

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        assert!(stdout.contains("3"));

        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 3);
        assert_eq!(_unique, 3);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_stats() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add some events
        mirror_log::log::append(&conn, "source1", "Event content 1", None)
            .expect("Failed to append");

        mirror_log::log::append(&conn, "source2", "Event content 2", None)
            .expect("Failed to append");

        mirror_log::log::append(
            &conn,
            "source1",
            "Event content 1", // duplicate
            None,
        )
        .expect("Failed to append duplicate");

        let (total, _unique, oldest, newest) =
            mirror_log::log::stats(&conn).expect("Failed to get stats");

        assert_eq!(total, 3);
        assert_eq!(_unique, 2);
        assert!(oldest > 0);
        assert!(newest > oldest);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_view_get_by_id() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let id = mirror_log::log::append(&conn, "view_test", "Content for viewing", None)
            .expect("Failed to append");

        let event = mirror_log::view::get_by_id(&conn, &id).expect("Failed to get event");

        assert_eq!(event.id, id);
        assert_eq!(event.source, "view_test");
        assert_eq!(event.content, "Content for viewing");

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_view_recent() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add multiple events
        for i in 1..=5 {
            mirror_log::log::append(&conn, "recent_test", &format!("Recent event {}", i), None)
                .expect("Failed to append");

            thread::sleep(Duration::from_millis(10)); // Small delay to get different timestamps
        }

        let events = mirror_log::view::recent(&conn, 3).expect("Failed to get recent events");

        assert_eq!(events.len(), 3);
        // Should be in descending order by ingestion time
        assert!(events[0].content.contains("5"));

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_view_by_source() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add events from different sources
        for i in 1..=5 {
            mirror_log::log::append(
                &conn,
                &format!("source_{}", i % 3), // Only 3 unique sources
                &format!("Content from source {}", i % 3),
                None,
            )
            .expect("Failed to append");
        }

        let events =
            mirror_log::view::by_source(&conn, "source_1", Some(10)).expect("Failed to get events");

        assert_eq!(events.len(), 2); // source_1 appears twice

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_search() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add events with search terms
        mirror_log::log::append(&conn, "search_test", "Find this text content", None)
            .expect("Failed to append");

        mirror_log::log::append(&conn, "search_test", "Different content here", None)
            .expect("Failed to append");

        mirror_log::log::append(&conn, "search_test", "Find this text again", None)
            .expect("Failed to append");

        let events = mirror_log::view::search(&conn, "Find this text").expect("Failed to search");

        assert_eq!(events.len(), 2);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_is_duplicate() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let content = "Test content";
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash = format!("{:x}", hasher.finalize());

        // Initially should be false
        let is_dup =
            mirror_log::log::is_duplicate(&conn, &content_hash).expect("Failed to check duplicate");
        assert!(!is_dup);

        // Add event with this content
        mirror_log::log::append(&conn, "duplicate_test", content, None).expect("Failed to append");

        // Now should be true
        let is_dup =
            mirror_log::log::is_duplicate(&conn, &content_hash).expect("Failed to check duplicate");
        assert!(is_dup);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_error_handling() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add an event
        mirror_log::log::append(&conn, "error_test", "Test error handling", None)
            .expect("Failed to append");

        // Try to get non-existent event
        let result = mirror_log::view::get_by_id(&conn, "non-existent-id");
        assert!(result.is_err());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_empty_db() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let (total, _unique, oldest, newest) =
            mirror_log::log::stats(&conn).expect("Failed to get stats");

        assert_eq!(total, 0);
        assert_eq!(_unique, 0);
        assert_eq!(oldest, 0);
        assert_eq!(newest, 0);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_large_content() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let large_content = "A".repeat(100000); // 100KB of content

        let id = mirror_log::log::append(&conn, "large_content_test", &large_content, None)
            .expect("Failed to append");

        // Verify it was stored correctly
        let event = mirror_log::view::get_by_id(&conn, &id).expect("Failed to get event");
        assert_eq!(event.content.len(), 100000);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_meta_field() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let meta = "test_meta_data";

        let _id = mirror_log::log::append(&conn, "meta_test", "Test content with meta", Some(meta))
            .expect("Failed to append");

        let large_content = "A".repeat(3000); // 3KB content that will be chunked

        let id = mirror_log::log::append(&conn, "chunk_test", &large_content, None)
            .expect("Failed to append");

        // Create chunks
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let chunk_count =
            mirror_log::chunk::create_chunks(&conn, &id, &large_content, timestamp, 1500)
                .expect("Failed to create chunks");

        assert!(chunk_count > 1); // Should create multiple chunks

        // Verify chunks exist
        let chunks = mirror_log::chunk::list_chunks(&conn, &id).expect("Failed to list chunks");
        assert_eq!(chunks.len(), chunk_count as usize);

        // Verify chunk content
        let total_chunks_content: usize = chunks.iter().map(|c| c.content.len()).sum();
        assert_eq!(total_chunks_content, large_content.len());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_chunk_search() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Create a chunk with specific text
        let content = "This is a test chunk content with specific text to search for";
        let id = mirror_log::log::append(&conn, "chunk_search_test", content, None)
            .expect("Failed to append");

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        mirror_log::chunk::create_chunks(&conn, &id, content, timestamp, 20)
            .expect("Failed to create chunks");

        // Search for text within chunks
        let search_term = "specific text";
        let found_chunks = mirror_log::chunk::search_chunks(&conn, search_term, Some(10))
            .expect("Failed to search chunks");

        assert!(!found_chunks.is_empty());
        // Should find at least one chunk containing our search term
        assert!(found_chunks.iter().any(|c| c.content.contains(search_term)));

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_attention_fresh_db_does_not_require_manual_init() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let items = mirror_log::AttentionLayer::default()
            .get_active_items(&conn)
            .expect("Fresh DB attention query should succeed");

        assert!(items.is_empty());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_ingest_single_rolls_back_when_chunk_creation_fails() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        conn.execute("DROP TABLE chunks", [])
            .expect("Failed to drop chunks table");

        let request = mirror_log::pipeline::IngestRequest::new(
            "atomic_test",
            "This content is long enough to require chunking",
            None,
        )
        .with_chunking(1, 10);

        let result = mirror_log::pipeline::ingest_single(&conn, None, request);
        assert!(result.is_err());

        let (total, unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 0);
        assert_eq!(unique, 0);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_shadowed_events_are_hidden_from_normal_queries_until_restored() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let visible_id = mirror_log::log::append(&conn, "shadow_test", "visible event", None)
            .expect("Failed to append visible event");
        let shadowed_id = mirror_log::log::append(&conn, "shadow_test", "shadowed event", None)
            .expect("Failed to append shadowed event");

        conn.execute(
            "INSERT INTO decay (event_id, access_count, last_accessed, pinned)
             VALUES (?1, 0, unixepoch() - 31 * 86400, 0)",
            [&shadowed_id],
        )
        .expect("Failed to seed decay state");

        let config = mirror_log::decay::DecayConfig::default();
        let moved = mirror_log::move_to_shadow(&conn, &config).expect("Failed to shadow events");
        assert_eq!(moved, 1);

        let recent = mirror_log::view::recent(&conn, 10).expect("Failed to load recent events");
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, visible_id);

        let shadowed = mirror_log::get_shadow_events(&conn).expect("Failed to load shadow events");
        assert_eq!(shadowed.len(), 1);
        assert_eq!(shadowed[0].id, shadowed_id);

        let (total, unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);
        assert_eq!(unique, 1);

        mirror_log::restore_from_shadow(&conn, &shadowed_id).expect("Failed to restore event");

        let restored = mirror_log::view::recent(&conn, 10).expect("Failed to load recent events");
        assert_eq!(restored.len(), 2);
        assert!(restored.iter().any(|event| event.id == shadowed_id));

        fs::remove_file(&db_path).ok();
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    fn temp_db() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push("mirror_log_cli_test_");
        let random: u64 = rand::random();
        path.push(format!("cli_test_{}.db", random));

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }

        path
    }

    fn get_binary_path() -> PathBuf {
        std::env::var("CARGO_BIN_EXE_mirror_log")
            .or_else(|_| std::env::var("CARGO_BIN_EXE_mirror-log"))
            .map(PathBuf::from)
            .or_else(|_| {
                let mut path =
                    std::env::current_exe().map_err(|_| std::env::VarError::NotPresent)?;
                path.pop();
                path.pop();
                path.push(format!("mirror-log{}", std::env::consts::EXE_SUFFIX));
                Ok::<PathBuf, std::env::VarError>(path)
            })
            .expect("Cargo binary path environment variable not set")
    }

    #[test]
    fn test_cli_add_basic() {
        let db_path = temp_db();
        let staging_dir = db_path.parent().unwrap().join("staging_add_basic");

        let child = Command::new(get_binary_path())
            .args([
                "--db",
                db_path.to_str().unwrap(),
                "add",
                "Test event content",
                "--source",
                "test",
            ])
            .current_dir(staging_dir.parent().unwrap())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        // `add` now stages events to disk rather than inserting into SQLite
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Staged:"),
            "Expected staging confirmation, got: {}",
            stdout
        );

        fs::remove_file(&db_path).ok();
        fs::remove_dir_all(staging_dir.parent().unwrap().join("staging")).ok();
    }

    #[test]
    fn test_cli_add_with_meta() {
        let db_path = temp_db();
        let child = Command::new(get_binary_path())
            .args([
                "--db",
                db_path.to_str().unwrap(),
                "add",
                "Test event with meta",
                "--source",
                "test",
                "--meta",
                r#"{"key": "value"}"#,
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        // `add` now stages events to disk rather than inserting into SQLite
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Staged:"),
            "Expected staging confirmation, got: {}",
            stdout
        );

        fs::remove_file(&db_path).ok();
        fs::remove_dir_all("staging").ok();
    }

    #[test]
    fn test_cli_add_file() {
        let db_path = temp_db();
        let mut file_path = temp_db();
        file_path.set_extension("txt");

        let mut file = fs::File::create(&file_path).expect("Failed to create test file");
        writeln!(file, "File content for testing").expect("Failed to write to file");

        let child = Command::new(get_binary_path())
            .args([
                "--db",
                db_path.to_str().unwrap(),
                "add-file",
                file_path.to_str().unwrap(),
                "--source",
                "test",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        // `add-file` now stages events to disk rather than inserting into SQLite
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Staged file:"),
            "Expected staging confirmation, got: {}",
            stdout
        );

        fs::remove_file(&db_path).ok();
        fs::remove_file(&file_path).ok();
        fs::remove_dir_all("staging").ok();
    }

    #[test]
    fn test_cli_show_basic() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add an event
        let _id =
            mirror_log::log::append(&conn, "test", "Test content", None).expect("Failed to append");

        let child = Command::new(get_binary_path())
            .args(["--db", db_path.to_str().unwrap(), "show", "--last", "5"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        assert!(stdout.contains("Test content"));

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_cli_show_by_source() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add events from different sources
        mirror_log::log::append(&conn, "source1", "Content from source1", None)
            .expect("Failed to append");
        mirror_log::log::append(&conn, "source2", "Content from source2", None)
            .expect("Failed to append");

        let child = Command::new(get_binary_path())
            .args([
                "--db",
                db_path.to_str().unwrap(),
                "show",
                "--source",
                "source1",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        assert!(stdout.contains("Content from source1"));

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_cli_search_basic() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with search term
        mirror_log::log::append(&conn, "test", "Search for this text", None)
            .expect("Failed to append");

        let child = Command::new(get_binary_path())
            .args([
                "--db",
                db_path.to_str().unwrap(),
                "search",
                "Search for this text",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        assert!(stdout.contains("Search for this text"));

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_cli_search_chunks() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with chunked content
        let content = "This is a test chunk content with specific text to search for";
        let id = mirror_log::log::append(&conn, "test", content, None).expect("Failed to append");

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        mirror_log::chunk::create_chunks(&conn, &id, content, timestamp, 20)
            .expect("Failed to create chunks");

        let child = Command::new(get_binary_path())
            .args([
                "--db",
                db_path.to_str().unwrap(),
                "search",
                "specific text",
                "--chunks",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        assert!(stdout.contains("specific text"));

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_cli_get_by_id() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add an event
        let id =
            mirror_log::log::append(&conn, "test", "Test content", None).expect("Failed to append");

        let child = Command::new(get_binary_path())
            .args(["--db", db_path.to_str().unwrap(), "get", &id])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        assert!(stdout.contains("Test content"));

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_cli_stats() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add some events
        mirror_log::log::append(&conn, "source1", "Event content 1", None)
            .expect("Failed to append");
        mirror_log::log::append(&conn, "source2", "Event content 2", None)
            .expect("Failed to append");

        let child = Command::new(get_binary_path())
            .args(["--db", db_path.to_str().unwrap(), "stats"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        assert!(stdout.contains("Total"));
        assert!(stdout.contains("Unique"));

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_cli_info() {
        let db_path = temp_db();
        let _conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let child = Command::new(get_binary_path())
            .args(["--db", db_path.to_str().unwrap(), "info"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        assert!(stdout.contains("Database"));

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_cli_stdin() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let input = "Stdin CLI test event\n";
        let mut child = Command::new(get_binary_path())
            .args([
                "--db",
                db_path.to_str().unwrap(),
                "stdin",
                "--source",
                "cli_stdin",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input.as_bytes())
                .expect("Failed to write to stdin");
        }

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_cli_duplicate_detection() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add same content twice
        let content = "Duplicate content for CLI test";
        mirror_log::log::append(&conn, "source1", content, None).expect("Failed to append");
        mirror_log::log::append(&conn, "source2", content, None).expect("Failed to append");

        let child = Command::new(get_binary_path())
            .args(["--db", db_path.to_str().unwrap(), "stats"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        assert!(stdout.contains("2"));

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_cli_help() {
        let db_path = temp_db();

        let child = Command::new(get_binary_path())
            .args(["--help"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        let output = child
            .wait_with_output()
            .expect("Failed to wait for process");
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        assert!(stdout.contains("Usage:"));
        assert!(stdout.contains("Commands:"));

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_cli_verify_ok() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");
        mirror_log::log::append(&conn, "verify_test", "Verify me", None).expect("Failed to append");

        let output = Command::new(get_binary_path())
            .args(["--db", db_path.to_str().unwrap(), "verify"])
            .output()
            .expect("Failed to run verify");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Status: OK"));

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_cli_verify_detects_hash_mismatch() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");
        let id = mirror_log::log::append(&conn, "verify_test", "Original content", None)
            .expect("Failed to append");

        conn.execute(
            "UPDATE events SET content = ?1 WHERE id = ?2",
            ["Tampered content", id.as_str()],
        )
        .expect("Failed to tamper row");

        let output = Command::new(get_binary_path())
            .args(["--db", db_path.to_str().unwrap(), "verify"])
            .output()
            .expect("Failed to run verify");

        assert!(!output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Hash mismatches: 1"));

        fs::remove_file(&db_path).ok();
    }
}
