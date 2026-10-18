//! Benchmark for mirror-log attention layer decision-making.
//! Measures latency and throughput of the rule-based attention approach.

use std::time::{Duration, Instant};
use tempfile::tempdir;

fn main() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("benchmark.db");
    
    println!("=== Mirror-Log Attention Layer Benchmark ===");
    println!("Database: {}", db_path.display());
    println!();
    
    // Initialize mirror-log database
    let conn = mirror_log::db::init_db(&db_path).unwrap();
    
    // Phase 1: Populate with test events
    println!("Phase 1: Populating test events...");
    let start = Instant::now();
    
    let test_events = vec![
        ("journal", "High GPU utilization on worker node ftw3 - 95% usage"),
        ("monitoring", "Memory pressure detected: swap usage at 80%"),
        ("alert", "Worker ftw3 unresponsive for 30 seconds"),
        ("journal", "Task queue depth increasing: 12 pending tasks"),
        ("metrics", "Network latency spike: avg 500ms vs normal 50ms"),
        ("journal", "Disk I/O bottleneck on storage node"),
        ("alert", "Service health check failed for API gateway"),
        ("monitoring", "CPU temperature rising on compute nodes"),
        ("journal", "Container restart loop detected for web service"),
        ("metrics", "Request error rate above 5% threshold"),
    ];
    
    let mut event_ids = Vec::new();
    for (source, content) in &test_events {
        let id = mirror_log::append(&conn, source, content, None).unwrap();
        
        // Initialize decay tracking so events appear in attention layer
        mirror_log::decay::track_access(&conn, &id).unwrap();
        
        event_ids.push(id);
    }
    
    let populate_time = start.elapsed();
    println!("  Inserted {} events in {:?}", test_events.len(), populate_time);
    println!();
    
    // Phase 2: Benchmark attention queries (retrieval)
    println!("Phase 2: Attention query benchmark...");
    let attention = mirror_log::AttentionLayer::default();
    
    // Warm up
    attention.get_active_items(&conn).unwrap();
    
    let iterations = 100;
    let start = Instant::now();
    for _ in 0..iterations {
        let items = attention.get_active_items(&conn).unwrap();
        assert!(!items.is_empty(), "Should have active items");
    }
    let query_time = start.elapsed();
    
    println!("  {} queries in {:?}", iterations, query_time);
    let avg_ns = query_time.as_nanos() / iterations as u128;
    println!("  Average: {}ns per query", avg_ns);
    println!();
    
    // Phase 3: Benchmark decay scoring
    println!("Phase 3: Decay scoring benchmark...");
    let start = Instant::now();
    for event_id in &event_ids {
        let _score = attention.calculate_attention_score(&conn, event_id);
    }
    let score_time = start.elapsed();
    println!("  Scored {} events in {:?}", event_ids.len(), score_time);
    println!();
    
    // Phase 4: Benchmark stats retrieval
    println!("Phase 4: Stats retrieval benchmark...");
    let iterations = 50;
    let start = Instant::now();
    for _ in 0..iterations {
        let stats = attention.get_stats(&conn).unwrap();
        assert!(stats.total_events > 0);
    }
    let stats_time = start.elapsed();
    println!("  {} stats queries in {:?}", iterations, stats_time);
    let avg_ns = stats_time.as_nanos() / iterations as u128;
    println!("  Average: {}ns per query", avg_ns);
    println!();
    
    // Summary
    println!("=== Benchmark Complete ===");
    println!("Attention layer uses rule-based temporal/access-frequency logic.");
    println!("No embedding API calls required - purely local SQLite operations.");
}
