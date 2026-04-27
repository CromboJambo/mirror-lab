use std::fs;
use std::path::PathBuf;

fn temp_db() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push("mirror_log_unicode_test_");
    let random: u64 = rand::random();
    path.push(format!("unicode_test_{}.db", random));

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }

    path
}

#[cfg(test)]
mod unicode_tests {
    use super::*;

    #[test]
    fn test_unicode_basic() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let unicode_content = "Hello 世界 🌍";
        let result = mirror_log::log::append(&conn, "unicode_test", unicode_content, None)
            .expect("Failed to append");

        assert!(!result.is_empty());

        // Verify retrieval
        let event = mirror_log::view::get_by_id(&conn, &result).expect("Failed to get event");
        assert_eq!(event.content, unicode_content);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_multiple_languages() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let multilingual_content = "Hello 世界 🌍\nBonjour monde 🌏\nHola mundo 🌎\n你好世界 🌏";
        let result =
            mirror_log::log::append(&conn, "multilingual_test", multilingual_content, None)
                .expect("Failed to append");

        assert!(!result.is_empty());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_chinese() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let chinese_content = "你好世界，这是一个测试。这是一个包含中文的测试内容。";
        let result = mirror_log::log::append(&conn, "chinese_test", chinese_content, None)
            .expect("Failed to append");

        assert!(!result.is_empty());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_arabic() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let arabic_content =
            "مرحبا بالعالم، هذا هو اختبار يحتوي على نص باللغة العربية. هذا نص اختبار آخر.";
        let result = mirror_log::log::append(&conn, "arabic_test", arabic_content, None)
            .expect("Failed to append");

        assert!(!result.is_empty());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_russian() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let russian_content =
            "Привет мир, это тест, содержащий текст на русском языке. Это ещё один тестовый текст.";
        let result = mirror_log::log::append(&conn, "russian_test", russian_content, None)
            .expect("Failed to append");

        assert!(!result.is_empty());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_emoticons_and_symbols() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let emoji_content =
            "🎉🎊🎈🎂🎁🎈🎊🎉\n❤️💖💗💕\n🌟⭐✨\n🚀🛸👽👾\n🎵🎶🎹🎸\n📚📖✍️\n🔥⚡️💥";
        let result = mirror_log::log::append(&conn, "emoji_test", emoji_content, None)
            .expect("Failed to append");

        assert!(!result.is_empty());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_cyrillic() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let cyrillic_content = "Привет, это тест с кириллицей. Тест содержит текст на русском языке. Это ещё одна строка.";
        let result = mirror_log::log::append(&conn, "cyrillic_test", cyrillic_content, None)
            .expect("Failed to append");

        assert!(!result.is_empty());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_special_chars() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let special_content = "Special characters: @#$%^&*()_+-=[]{}|;':\",./<>?\n~`!@#$%^&*()_+-=[]{}|;':\",./<>?\n¡¿§¶•ªº«»";
        let result = mirror_log::log::append(&conn, "special_test", special_content, None)
            .expect("Failed to append");

        assert!(!result.is_empty());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_long_text_with_various_scripts() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let long_content = "This is a long text with various scripts: Hello 世界 🌍, Bonjour monde 🌏, Hola mundo 🌎, 你好世界 🌏, Привет мир 🌍.";

        let result = mirror_log::log::append(&conn, "long_unicode_test", long_content, None)
            .expect("Failed to append");

        assert!(!result.is_empty());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_duplicate_detection() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let unicode_content = "Hello 世界 🌍";

        // First append
        mirror_log::log::append(&conn, "source1", unicode_content, None).expect("Failed to append");

        // Second append
        mirror_log::log::append(&conn, "source2", unicode_content, None).expect("Failed to append");

        let (total, _unique, _, _) = mirror_log::log::stats(&conn).expect("Failed to get stats");

        // Note: This test depends on duplicate detection working with unicode
        // The README says duplicates are allowed but hash-based lookup remains
        assert_eq!(total, 2);

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_search() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Add event with unicode content
        let unicode_content = "Search for 世界 🌍 in this unicode text";
        let _result = mirror_log::log::append(&conn, "search_unicode_test", unicode_content, None)
            .expect("Failed to append");

        // Search for unicode content
        let events = mirror_log::view::search(&conn, "世界").expect("Failed to search");

        assert!(!events.is_empty());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_large_content() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Create large unicode content
        let mut large_content = String::new();
        for i in 0..100 {
            large_content.push_str(&format!(
                "这是一段测试文本编号 {}。这个文本包含中文字符。这是一个测试内容。",
                i
            ));
            large_content.push(' ');
        }

        let id = mirror_log::log::append(&conn, "large_unicode_test", &large_content, None)
            .expect("Failed to append");

        // Verify it was stored correctly
        let event = mirror_log::view::get_by_id(&conn, &id).expect("Failed to get event");
        assert_eq!(event.content.chars().count(), large_content.chars().count());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_meta_field() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let unicode_content = "Test content with unicode 🌍";
        let unicode_meta = r#"{"key": "你好世界", "emoji": "🌍", "language": "中文"}"#;

        let _id = mirror_log::log::append(
            &conn,
            "meta_unicode_test",
            unicode_content,
            Some(unicode_meta),
        )
        .expect("Failed to append");

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_chunking() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Create content that will be chunked
        let content = "这是一个测试文本，将会被分割成多个块。这个文本包含中文字符和英文混合。这是一个很长的文本，用于测试分块功能。";
        let id = mirror_log::log::append(&conn, "chunk_unicode_test", content, None)
            .expect("Failed to append");

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let chunk_count = mirror_log::chunk::create_chunks(&conn, &id, content, timestamp, 20)
            .expect("Failed to create chunks");

        assert!(chunk_count > 1); // Should create multiple chunks

        // Verify chunks exist
        let chunks = mirror_log::chunk::list_chunks(&conn, &id).expect("Failed to list chunks");
        assert!(!chunks.is_empty());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_chunk_search() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Create a chunk with unicode content
        let content = "这是一个测试块内容，用于搜索特定文本。这个块包含中文字符。这是一个测试块内容，用于搜索特定文本。";
        let id = mirror_log::log::append(&conn, "chunk_search_unicode_test", content, None)
            .expect("Failed to append");

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        mirror_log::chunk::create_chunks(&conn, &id, content, timestamp, 20)
            .expect("Failed to create chunks");

        // Search for unicode text within chunks
        let search_term = "测试";
        let found_chunks = mirror_log::chunk::search_chunks(&conn, search_term, Some(10))
            .expect("Failed to search chunks");

        assert!(!found_chunks.is_empty());
        assert!(found_chunks.iter().any(|c| c.content.contains(search_term)));

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_empty_content() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        // Empty unicode content
        let empty_content = "";
        let result = mirror_log::log::append(&conn, "empty_unicode_test", empty_content, None);

        // Should handle empty content gracefully
        assert!(result.is_ok());

        fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_unicode_multiline_with_various_scripts() {
        let db_path = temp_db();
        let conn = mirror_log::db::init_db(&db_path).expect("Failed to initialize DB");

        let multiline_content = "Line 1: Hello world\nLine 2: 你好世界\nLine 3: Bonjour le monde\nLine 4: مرحبا بالعالم\nLine 5: Привет мир";

        let result =
            mirror_log::log::append(&conn, "multiline_unicode_test", multiline_content, None)
                .expect("Failed to append");

        assert!(!result.is_empty());

        fs::remove_file(&db_path).ok();
    }
}
