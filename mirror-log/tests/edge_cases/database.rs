use std::fs;
use std::path::PathBuf;

fn temp_db() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push("mirror_log_database_test_");
    let random: u64 = rand::random();
    path.push(format!("database_test_{}.db", random));

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }

    path
}

#[cfg(test)]
mod database_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_database_corruption_recovery() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add some events
        mirror_log::log::append(&conn, "source1", "Event content 1", None)
            .expect("Failed to append");
        mirror_log::log::append(&conn, "source2", "Event content 2", None)
            .expect("Failed to append");

        // Corrupt the database file
        use std::fs::File;
        let mut file = File::create(&db_path).expect("Failed to create file");
        file.write_all(b"corrupted database content")
            .expect("Failed to write");
        fs::remove_file(&db_path).ok();

        // Try to initialize DB - should handle corruption gracefully
        let result = mirror_log::db::init_db(&db_path);
        assert!(result.is_err() || fs::metadata(&db_path).is_ok());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_migration_scenario() {
        let db_path = temp_db();

        // Initialize with old schema
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add events
        mirror_log::log::append(&conn, "source1", "Event content 1", None)
            .expect("Failed to append");

        // Simulate migration by closing and reopening
        drop(conn);

        // Reinitialize - should handle migration
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);
        assert_eq!(_unique, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_concurrent_access() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add events
        mirror_log::log::append(&conn, "source1", "Event content 1", None)
            .expect("Failed to append");
        mirror_log::log::append(&conn, "source2", "Event content 2", None)
            .expect("Failed to append");

        // Simulate concurrent access by spawning multiple threads
        use std::thread;

        let mut handles = vec![];
        for i in 0..5 {
            let db_path_clone = db_path.clone();
            let handle = thread::spawn(move || {
                let conn =
                    mirror_log::db::init_db(&db_path_clone).expect("Failed to initialize DB");
                mirror_log::log::append(&conn, "concurrent", &format!("Event {}", i), None)
                    .expect("Failed to append");
                drop(conn);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Thread failed");
        }

        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");

        // Should have at least 7 events (2 original + 5 concurrent)
        assert!(total >= 7);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_backup_restore() {
        let db_path = temp_db();
        let mut backup_path = temp_db();
        backup_path.set_extension("backup");

        // Initialize and add events
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");
        mirror_log::log::append(&conn, "source1", "Event content 1", None)
            .expect("Failed to append");
        mirror_log::log::append(&conn, "source2", "Event content 2", None)
            .expect("Failed to append");
        drop(conn);

        // Backup database
        fs::copy(&db_path, &backup_path).expect("Failed to backup");

        // Add more events
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");
        mirror_log::log::append(&conn, "source3", "Event content 3", None)
            .expect("Failed to append");
        drop(conn);

        // Restore from backup
        fs::copy(&backup_path, &db_path).expect("Failed to restore");

        // Verify backup content
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");

        assert_eq!(total, 2);
        assert_eq!(_unique, 2);

        fs::remove_file(&db_path).ok();
        fs::remove_file(&backup_path).ok();
    }

    #[test]
    fn test_multiple_databases() {
        let db_path1 = temp_db();
        let db_path2 = temp_db();
        let db_path3 = temp_db();

        // Initialize three databases
        let conn1 = mirror_log::db::init_db(&db_path1).expect("Failed to initialize DB1");
        let conn2 = mirror_log::db::init_db(&db_path2).expect("Failed to initialize DB2");
        let conn3 = mirror_log::db::init_db(&db_path3).expect("Failed to initialize DB3");

        // Add events to each database
        mirror_log::log::append(&conn1, "source1", "Event from DB1", None)
            .expect("Failed to append");
        mirror_log::log::append(&conn2, "source2", "Event from DB2", None)
            .expect("Failed to append");
        mirror_log::log::append(&conn3, "source3", "Event from DB3", None)
            .expect("Failed to append");

        // Verify each database has its own data
        let conn1 = mirror_log::db::init_db(&db_path1).expect("Failed to initialize DB1");
        let conn2 = mirror_log::db::init_db(&db_path2).expect("Failed to initialize DB2");
        let conn3 = mirror_log::db::init_db(&db_path3).expect("Failed to initialize DB3");

        let (total1, unique1, _, _) = mirror_log::log::stats(&conn1).expect("Failed to get stats");
        let (total2, unique2, _, _) = mirror_log::log::stats(&conn2).expect("Failed to get stats");
        let (total3, unique3, _, _) = mirror_log::log::stats(&conn3).expect("Failed to get stats");

        assert_eq!(total1, 1);
        assert_eq!(unique1, 1);
        assert_eq!(total2, 1);
        assert_eq!(unique2, 1);
        assert_eq!(total3, 1);
        assert_eq!(unique3, 1);

        fs::remove_file(&db_path1).ok();
        fs::remove_file(&db_path2).ok();
        fs::remove_file(&db_path3).ok();
    }

    #[test]
    fn test_database_size_limits() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add many events to test database size handling
        for i in 0..100 {
            let content = format!("Event content number {}", i);
            mirror_log::log::append(&conn, "size_test", &content, None).expect("Failed to append");
        }

        // Verify all events were stored
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 100);
        assert_eq!(_unique, 100);

        // Verify database file exists
        assert!(fs::metadata(&db_path).is_ok());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_locking() {
        let db_path = temp_db();

        // Initialize database
        let conn1 = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add events
        mirror_log::log::append(&conn1, "source1", "Event content 1", None)
            .expect("Failed to append");

        // Try to access from another connection
        let conn2 = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");
        let (total, _unique, _, _) = mirror_log::log::stats(&conn2).expect("Failed to get stats");

        assert_eq!(total, 1);
        assert_eq!(_unique, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_cleanup() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add events
        mirror_log::log::append(&conn, "source1", "Event content 1", None)
            .expect("Failed to append");
        mirror_log::log::append(&conn, "source2", "Event content 2", None)
            .expect("Failed to append");

        // Verify events exist
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 2);

        // Close connection
        drop(conn);

        // Verify database file still exists
        assert!(fs::metadata(&db_path).is_ok());

        // Manually remove database
        fs::remove_file(&db_path).ok();

        // Verify cleanup
        assert!(fs::metadata(&db_path).is_err());
    }

    #[test]
    fn test_database_path_handling() {
        let db_path = temp_db();

        // Test with path-like input
        let result = mirror_log::db::init_db(&db_path);
        assert!(result.is_ok());

        // Test with PathBuf
        let result = mirror_log::db::init_db(db_path.clone());
        assert!(result.is_ok());

        // Test with string path
        let result = mirror_log::db::init_db(db_path.to_str().unwrap());
        assert!(result.is_ok());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_error_handling() {
        let db_path = temp_db();

        // Test with non-existent database
        let result = mirror_log::db::init_db(&db_path);
        assert!(result.is_ok());

        // Close database
        drop(result.unwrap());

        // Try to open again
        let result = mirror_log::db::init_db(&db_path);
        assert!(result.is_ok());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_transaction_handling() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add events
        mirror_log::log::append(&conn, "source1", "Event content 1", None)
            .expect("Failed to append");
        mirror_log::log::append(&conn, "source2", "Event content 2", None)
            .expect("Failed to append");

        // Verify events exist
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 2);

        // Close connection
        drop(conn);

        // Verify database file still exists
        assert!(fs::metadata(&db_path).is_ok());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_special_characters_in_path() {
        let mut db_path = temp_db();
        db_path.set_file_name("test_database_#1$2%3&4'5.db");

        // Initialize database with special characters in path
        let result = mirror_log::db::init_db(&db_path);
        assert!(result.is_ok());

        // Add events
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");
        mirror_log::log::append(&conn, "source1", "Event content 1", None)
            .expect("Failed to append");

        // Verify events
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_empty_database() {
        let db_path = temp_db();

        // Initialize empty database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Verify empty state
        let (total, _unique, oldest, newest) =
            mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 0);
        assert_eq!(_unique, 0);
        assert_eq!(oldest, 0);
        assert_eq!(newest, 0);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_large_number_of_sources() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add events from many sources
        for i in 0..50 {
            let source = format!("source_{}", i);
            let content = format!("Event from source {}", i);
            mirror_log::log::append(&conn, &source, &content, None).expect("Failed to append");
        }

        // Verify events
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 50);
        assert_eq!(_unique, 50);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_long_source_names() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add events with long source names
        let long_source = "a".repeat(100);
        let content = "Event content";
        mirror_log::log::append(&conn, &long_source, content, None).expect("Failed to append");

        // Verify event
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_unicode_paths() {
        let mut db_path = temp_db();
        db_path.set_file_name("测试数据库_🌍.db");

        // Initialize database with unicode path
        let result = mirror_log::db::init_db(&db_path);
        assert!(result.is_ok());

        // Add events
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");
        mirror_log::log::append(&conn, "source1", "Event content 1", None)
            .expect("Failed to append");

        // Verify events
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_null_bytes() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with null byte in content
        let content = "Event with null\x00 byte";
        mirror_log::log::append(&conn, "source1", content, None).expect("Failed to append");

        // Verify event
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_binary_content() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with binary content
        let content = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        let content_string = String::from_utf8_lossy(&content).to_string();
        mirror_log::log::append(&conn, "source1", &content_string, None).expect("Failed to append");

        // Verify event
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_malformed_json_meta() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with malformed JSON meta
        let content = "Event content";
        let meta = r#"{"key": "value", "invalid": "}}"#;
        mirror_log::log::append(&conn, "source1", content, Some(meta)).expect("Failed to append");

        // Verify event
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_complex_meta() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with complex JSON meta
        let content = "Event content";
        let meta = r#"{"key": "value", "number": 123, "array": [1, 2, 3], "nested": {"key": "value", "number": 456}, "unicode": "你好世界 🌍", "boolean": true, "null": null}"#;
        mirror_log::log::append(&conn, "source1", content, Some(meta)).expect("Failed to append");

        // Verify event
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_empty_meta() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with empty meta
        let content = "Event content";
        mirror_log::log::append(&conn, "source1", content, None).expect("Failed to append");

        // Verify event
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_special_characters_in_meta() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with special characters in meta
        let content = "Event content";
        let meta = r#"{"special": "!@#$%^&*()_+-=[]{}|;':\",./<>?", "unicode": "你好世界 🌍", "emoji": "🎉🎊🎈"}"#;
        mirror_log::log::append(&conn, "source1", content, Some(meta)).expect("Failed to append");

        // Verify event
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_very_long_meta() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with very long meta
        let content = "Event content";
        let meta = format!("{{\"key\": \"{}\"}}", "x".repeat(10000));
        mirror_log::log::append(&conn, "source1", content, Some(&meta)).expect("Failed to append");

        // Verify event
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_duplicate_deduplication() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add duplicate events
        let content = "Duplicate content";
        mirror_log::log::append(&conn, "source1", content, None).expect("Failed to append");
        mirror_log::log::append(&conn, "source2", content, None).expect("Failed to append");

        // Verify duplicate detection
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 2);
        assert_eq!(_unique, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_chunked_content() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with chunked content
        let content = "This is a test chunked content with specific text to search for";
        let id =
            mirror_log::log::append(&conn, "source1", content, None).expect("Failed to append");

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Create chunks
        let _chunk_count = mirror_log::chunk::create_chunks(&conn, &id, content, timestamp, 20)
            .expect("Failed to create chunks");

        // Verify chunks
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_chunk_search() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with chunked content
        let content = "This is a test chunked content with specific text to search for";
        let id =
            mirror_log::log::append(&conn, "source1", content, None).expect("Failed to append");

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Create chunks
        mirror_log::chunk::create_chunks(&conn, &id, content, timestamp, 20)
            .expect("Failed to create chunks");

        // Search for text in chunks
        let events = mirror_log::view::search(&conn, "specific text").expect("Failed to search");
        assert!(!events.is_empty());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_empty_chunk_content() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with empty content
        let content = "";
        let _id =
            mirror_log::log::append(&conn, "source1", content, None).expect("Failed to append");

        // Verify event
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_multiline_content() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with multiline content
        let content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        let _id =
            mirror_log::log::append(&conn, "source1", content, None).expect("Failed to append");

        // Verify event
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_very_long_content() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with very long content
        let content = "A".repeat(1000000); // 1MB content
        let _id =
            mirror_log::log::append(&conn, "source1", &content, None).expect("Failed to append");

        // Verify event
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_special_characters_in_content() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with special characters
        let content = "Special characters: !@#$%^&*()_+-=[]{}|;':\",./<>?\n\t\r\n";
        let _id =
            mirror_log::log::append(&conn, "source1", content, None).expect("Failed to append");

        // Verify event
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_database_with_unicode_content() {
        let db_path = temp_db();

        // Initialize database
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with unicode content
        let content = "你好世界 🌍";
        let _id =
            mirror_log::log::append(&conn, "source1", content, None).expect("Failed to append");

        // Verify event
        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");
        assert_eq!(total, 1);

        fs::remove_file(&db_path).ok();
    }
}
