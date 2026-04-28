use chrono::DateTime;
use chrono::TimeZone;
use chrono::Utc;
use clap::{Parser, Subcommand};
use mirror_log::stage::StagedEvent;
use mirror_log::{chunk, db, infer, log, pipeline, view};
use std::path::{Path, PathBuf};
#[cfg(feature = "embedding")]
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "mirror-log")]
#[command(about = "Append-only event log with SQLite", long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "mirror.db")]
    db: PathBuf,

    #[arg(short, long, default_value_t = 1000)]
    batch_size: usize,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show your attention layer (recently accessed events)
    Attention {
        /// Show flagged items (due for decay)
        #[arg(short, long)]
        flagged: bool,

        /// Show statistics
        #[arg(short, long)]
        stats: bool,
    },

    /// Add an event to the log
    Add {
        /// The content to log
        content: String,

        #[arg(short, long, default_value = "cli")]
        source: String,

        #[arg(short, long)]
        meta: Option<String>,
    },

    /// Add a file's contents as a single event
    AddFile {
        /// Path to the file
        path: PathBuf,

        #[arg(short, long, default_value = "file")]
        source: String,

        #[arg(short, long)]
        meta: Option<String>,
    },

    /// Add events from stdin (one per line)
    Stdin {
        #[arg(short, long, default_value = "stdin")]
        source: String,

        #[arg(short, long)]
        meta: Option<String>,
    },

    /// Show ingestion statistics
    Stats,

    /// Show recent events
    Show {
        #[arg(short, long, default_value_t = 20)]
        last: i64,

        #[arg(short, long)]
        source: Option<String>,

        #[arg(short, long)]
        preview: Option<usize>,
    },

    /// Search events by content
    Search {
        /// Search term
        term: String,

        #[arg(short, long)]
        preview: Option<usize>,

        #[arg(long)]
        chunks: bool,
    },

    /// Get a specific event by ID
    Get {
        /// Event ID
        id: String,
    },

    /// Show database info
    Info,

    /// Verify database integrity invariants
    Verify,

    /// Generate embeddings for events in a source (optional feature)
    #[cfg(feature = "embedding")]
    Embed {
        #[arg(short, long, default_value = "cli")]
        source: String,

        #[arg(long, default_value = "token-bucket")]
        model: String,
    },

    /// Search similar events using embeddings (optional feature)
    #[cfg(feature = "embedding")]
    SearchSimilar {
        /// Search term (used to generate query vector)
        term: String,

        #[arg(long, default_value_t = 10)]
        limit: usize,
    },

    /// Add an event to the attention layer
    AddToAttention {
        /// Event ID to add to attention
        event_id: String,
    },

    /// Detect patterns from staged events and propose reflections
    Infer,

    /// Review staged events pending approval
    Review,

    /// Regenerate human.md from declarative base and approved reflections
    Regenerate {
        #[arg(long, default_value = "human.md")]
        output: String,
    },
}

#[cfg(feature = "embedding")]
use tokenizers::Tokenizer;
#[cfg(feature = "embedding")]
use tokenizers::models::bpe::BPE;

#[cfg(feature = "embedding")]
fn load_tokenizer(model: &str) -> Result<Tokenizer, String> {
    let path = std::path::Path::new(model);
    if path.exists() {
        return Tokenizer::from_file(path).map_err(|e| e.to_string());
    }

    Ok(Tokenizer::new(BPE::default()))
}

fn main() {
    let cli = Cli::parse();
    let db_path = cli.db.clone();
    let conn = db::init_db(&db_path).expect("Failed to open database");

    match cli.command {
        Commands::Add {
            content,
            source,
            meta,
        } => {
            let event = StagedEvent::new(&source, &content, meta.as_deref());
            let staging_dir = Path::new("staging");
            event
                .save_to_file(staging_dir)
                .expect("Failed to write staged event");

            println!("Staged: {} (waiting for approval)", event.id);
        }

        Commands::AddFile { path, source, meta } => {
            let content = std::fs::read_to_string(&path).expect("Failed to read file");
            let event = StagedEvent::new(&source, &content, meta.as_deref());
            let staging_dir = Path::new("staging");
            event
                .save_to_file(staging_dir)
                .expect("Failed to write staged event");

            println!("Staged file: {} ({})", path.display(), event.id);
        }

        Commands::Stdin { source, meta } => {
            let staging_dir = Path::new("staging");
            match pipeline::ingest_stdin_with_policy(
                &conn,
                &source,
                meta.as_deref(),
                cli.batch_size,
                pipeline::AUTO_CHUNK_THRESHOLD,
                pipeline::DEFAULT_CHUNK_SIZE,
            ) {
                Ok(result) => {
                    for event_id in &result.event_ids {
                        let event = view::get_by_id(&conn, event_id)
                            .expect("Failed to get event from temporary buffer");
                        let staged_event =
                            StagedEvent::new(&source, &event.content, event.meta.as_deref());
                        staged_event
                            .save_to_file(staging_dir)
                            .expect("Failed to write staged event");
                    }
                    println!(
                        "Staged {} events (waiting for approval)",
                        result.event_ids.len()
                    );
                }
                Err(e) => {
                    eprintln!("Failed to read from stdin: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Stats => {
            let (total, unique, _, latest) = log::stats(&conn).expect("Failed to get stats");

            println!("Ingestion Statistics:");
            println!("  Total events: {}", total);
            println!("  Unique: {}", unique);

            if total > 0 {
                let newest_dt: DateTime<Utc> = Utc.timestamp_opt(latest, 0).unwrap();
                println!("  Newest: {}", newest_dt.format("%Y-%m-%d %H:%M:%S UTC"));
            }
        }

        Commands::Show {
            last,
            source,
            preview,
        } => {
            let events = if let Some(src) = source {
                view::by_source(&conn, &src, Some(last)).expect("Failed to query events")
            } else {
                view::recent(&conn, last).expect("Failed to query events")
            };

            if events.is_empty() {
                println!("No events found");
            } else {
                for event in events {
                    println!("\n[{}] {}", event.format_time(), event.source);
                    println!("ID: {}", event.id);

                    if let Some(max_chars) = preview {
                        println!("{}", event.preview_content(max_chars));
                    } else {
                        println!("{}", event.content);
                    }

                    if let Some(meta) = event.meta {
                        println!("Meta: {}", meta);
                    }
                }
            }
        }

        Commands::Search {
            term,
            preview,
            chunks,
        } => {
            if chunks {
                let found_chunks =
                    chunk::search_chunks(&conn, &term, Some(20)).expect("Failed to search chunks");

                if found_chunks.is_empty() {
                    println!("No chunks found matching '{}'", term);
                } else {
                    println!("Found {} chunks:\n", found_chunks.len());
                    for chunk in found_chunks {
                        let event = view::get_by_id(&conn, &chunk.event_id)
                            .expect("Failed to get parent event");

                        println!(
                            "[{}] {} (chunk {}/...)",
                            event.format_time(),
                            event.source,
                            chunk.chunk_index + 1
                        );
                        println!("Event ID: {}", event.id);
                        println!("Chunk ID: {}", chunk.id);

                        if let Some(max_chars) = preview {
                            let total_chars = chunk.content.chars().count();
                            if total_chars > max_chars {
                                let preview_text: String =
                                    chunk.content.chars().take(max_chars).collect();
                                println!(
                                    "{}...\n[{} of {} chars]",
                                    preview_text, max_chars, total_chars
                                );
                            } else {
                                println!("{}", chunk.content);
                            }
                        } else {
                            println!("{}", chunk.content);
                        }

                        if let Some(meta) = event.meta {
                            println!("Meta: {}", meta);
                        }
                        println!();
                    }
                }
            } else {
                let events = view::search(&conn, &term).expect("Failed to search events");

                if events.is_empty() {
                    println!("No events found matching '{}'", term);
                } else {
                    println!("Found {} events:\n", events.len());
                    for event in events {
                        println!("[{}] {}", event.format_time(), event.source);
                        println!("ID: {}", event.id);

                        if let Some(max_chars) = preview {
                            println!("{}", event.preview_content(max_chars));
                        } else {
                            println!("{}", event.content);
                        }

                        if let Some(meta) = event.meta {
                            println!("Meta: {}", meta);
                        }
                        println!();
                    }
                }
            }
        }

        Commands::Get { id } => match view::get_by_id(&conn, &id) {
            Ok(event) => {
                println!("\n[{}] {}", event.format_time(), event.source);
                println!("ID: {}", event.id);
                println!("{}", event.content);
                if let Some(meta) = event.meta {
                    println!("Meta: {}", meta);
                }
            }
            Err(e) => {
                eprintln!("Event not found: {}", e);
                std::process::exit(1);
            }
        },

        Commands::Info => {
            let (count, oldest, newest) = db::db_info(&conn).expect("Failed to get database info");

            println!("Database Info:");
            println!("  Path: {}", db_path.display());
            println!("  Total events: {}", count);

            if count > 0 {
                let oldest_dt: DateTime<Utc> = Utc.timestamp_opt(oldest, 0).unwrap();
                let newest_dt: DateTime<Utc> = Utc.timestamp_opt(newest, 0).unwrap();

                println!("  Oldest: {}", oldest_dt.format("%Y-%m-%d %H:%M:%S UTC"));
                println!("  Newest: {}", newest_dt.format("%Y-%m-%d %H:%M:%S UTC"));
            }
        }

        Commands::Verify => {
            let report = log::verify_integrity(&conn).expect("Failed to verify database integrity");
            let issues =
                report.missing_or_invalid_hashes + report.hash_mismatches + report.orphan_chunks;

            println!("Integrity Report:");
            println!("  Total events: {}", report.total_events);
            println!(
                "  Missing/invalid hashes: {}",
                report.missing_or_invalid_hashes
            );
            println!("  Hash mismatches: {}", report.hash_mismatches);
            println!("  Orphan chunks: {}", report.orphan_chunks);

            if issues == 0 {
                println!("  Status: OK");
            } else {
                println!("  Status: FAILED ({} issues)", issues);
                std::process::exit(1);
            }
        }

        #[cfg(feature = "embedding")]
        Commands::Embed { source, model } => {
            let conn = db::init_db(&db_path).expect("Failed to open database");
            let tokenizer = match load_tokenizer(&model) {
                Ok(tokenizer) => tokenizer,
                Err(e) => {
                    eprintln!("Failed to load tokenizer '{}': {}", model, e);
                    std::process::exit(1);
                }
            };

            let provider = Arc::new(mirror_log::embedding::BaselineProvider::new(
                tokenizer, 512, &model,
            ));
            match mirror_log::embedding::EmbeddingService::init_from_path(&db_path, provider) {
                Ok(service) => {
                    let events =
                        view::by_source(&conn, &source, None).expect("Failed to query events");

                    if events.is_empty() {
                        println!("No events found for source: {}", source);
                        std::process::exit(0);
                    }

                    println!("Generating embeddings for {} events...", events.len());

                    let mut success_count = 0;
                    let mut error_count = 0;

                    for event in events {
                        match service.generate_embedding(&event.content) {
                            Ok(embedding) => {
                                if let Err(e) = service.store_embedding(&embedding, &event.id) {
                                    eprintln!(
                                        "Failed to store embedding for event {}: {}",
                                        event.id, e
                                    );
                                    error_count += 1;
                                } else {
                                    success_count += 1;
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "Failed to generate embedding for event {}: {}",
                                    event.id, e
                                );
                                error_count += 1;
                            }
                        }
                    }

                    println!("Embedding generation complete:");
                    println!("  Success: {}", success_count);
                    println!("  Errors: {}", error_count);

                    if success_count > 0 {
                        let stats = match service.get_embedding_stats() {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("Failed to get embedding stats: {}", e);
                                std::process::exit(1);
                            }
                        };
                        println!("  Total embeddings: {}", stats.total_embeddings);
                        println!("  Total events with embeddings: {}", stats.total_events);
                        println!(
                            "  Average vector length: {:.2}",
                            stats.average_vector_length
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Failed to initialize embedding service: {}", e);
                    std::process::exit(1);
                }
            }
        }

        #[cfg(feature = "embedding")]
        Commands::SearchSimilar { term, limit } => {
            let conn = db::init_db(&db_path).expect("Failed to open database");
            let tokenizer = Tokenizer::new(BPE::default());

            let provider = Arc::new(mirror_log::embedding::BaselineProvider::new(
                tokenizer,
                512,
                "token-bucket",
            ));
            match mirror_log::embedding::EmbeddingService::init_from_path(&db_path, provider) {
                Ok(service) => {
                    println!("Searching for similar events to: '{}'", term);
                    let query_embedding = match service.generate_embedding(&term) {
                        Ok(embedding) => embedding,
                        Err(e) => {
                            eprintln!("Failed to generate query embedding: {}", e);
                            std::process::exit(1);
                        }
                    };

                    let similarities = match service.search_similar(&query_embedding.vector, limit)
                    {
                        Ok(similarities) => similarities,
                        Err(e) => {
                            eprintln!("Failed to search similar events: {}", e);
                            std::process::exit(1);
                        }
                    };

                    if similarities.is_empty() {
                        println!("No similar events found");
                        return;
                    }

                    println!("Found {} similar events:\n", similarities.len());

                    for similarity in similarities {
                        match view::get_by_id(&conn, &similarity.event_id) {
                            Ok(event) => {
                                println!("[{}] {}", event.format_time(), event.source);
                                println!("ID: {}", event.id);
                                println!("Similarity Score: {:.4}", similarity.score);
                                println!("{}", event.preview_content(200));

                                if let Some(meta) = event.meta {
                                    println!("Meta: {}", meta);
                                }
                                println!();
                            }
                            Err(_) => {
                                println!("Event ID {} not found", similarity.event_id);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to initialize embedding service: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Attention { flagged, stats } => {
            if stats {
                let attention_stats = mirror_log::AttentionLayer::default()
                    .get_stats(&conn)
                    .expect("Failed to get attention stats");
                println!("Attention Statistics:");
                println!("  Total events: {}", attention_stats.total_events);
                println!("  Active events: {}", attention_stats.active_events);
                println!("  Pinned events: {}", attention_stats.pinned_events);
                println!("  Flagged events: {}", attention_stats.flagged_events);
                println!(
                    "  Active percentage: {:.2}%",
                    attention_stats.active_percentage()
                );
            } else if flagged {
                let flagged_items = mirror_log::AttentionLayer::default()
                    .get_flagged_items(&conn)
                    .expect("Failed to get flagged items");
                if flagged_items.is_empty() {
                    println!("No flagged events");
                } else {
                    println!("Flagged events (due for decay):");
                    for item in flagged_items {
                        println!("\n[{}] {}", item.last_accessed_str(), item.source);
                        println!("ID: {}", item.id);
                        println!("Content: {}", item.content);
                        println!("Access count: {}", item.access_count);
                    }
                }
            } else {
                let active_items = mirror_log::AttentionLayer::default()
                    .get_active_items(&conn)
                    .expect("Failed to get active items");
                if active_items.is_empty() {
                    println!("No active attention items");
                } else {
                    println!("Active attention items:");
                    for item in active_items {
                        println!("\n[{}] {}", item.last_accessed_str(), item.source);
                        println!("ID: {}", item.id);
                        println!("Content: {}", item.content);
                        println!("Access count: {}", item.access_count);
                        if let Some(meta) = &item.meta {
                            println!("Meta: {}", meta);
                        }
                    }
                }
            }
        }

        Commands::AddToAttention { event_id } => {
            match mirror_log::AttentionLayer::default().add_to_attention(&conn, &event_id) {
                Ok(_) => println!("Added event to attention: {}", event_id),
                Err(e) => {
                    eprintln!("Failed to add event to attention: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Infer => {
            let staging_dir = Path::new("staging");

            if !staging_dir.exists() {
                println!("No staging directory found. Stage events first with `mirror-log add`.");
                return;
            }

            match infer::detect_patterns(staging_dir) {
                Ok(patterns) => {
                    if patterns.is_empty() {
                        println!("No patterns detected from staged events.");
                    } else {
                        println!("Detected {} pattern(s):\n", patterns.len());
                        for pattern in &patterns {
                            println!("{}", pattern.description);
                            if !pattern.source_events.is_empty() {
                                println!("  Source events: {}", pattern.source_events.join(", "));
                            }
                            println!();
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to detect patterns: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Review => {
            let staging_dir = Path::new("staging");

            match StagedEvent::load_all(staging_dir) {
                Ok(events) => {
                    if events.is_empty() {
                        println!("No staged events found");
                    } else {
                        println!("Found {} staged event(s):\n", events.len());
                        for event in &events {
                            println!(
                                "[{}] {} ({})",
                                event.id,
                                event.source,
                                event.timestamp_utc().format("%Y-%m-%d %H:%M:%S UTC")
                            );
                            println!("  Content: {}", event.content);
                            if let Some(meta) = &event.meta {
                                println!("  Meta: {}", meta);
                            }
                            println!();
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read staging directory: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Regenerate { output } => {
            let staging_dir = Path::new("staging");

            match StagedEvent::load_all(staging_dir) {
                Ok(events) => {
                    if events.is_empty() {
                        println!("No staged events found — nothing to regenerate.");
                    } else {
                        println!("Regenerating {} with {} event(s)...", output, events.len());

                        for event in &events {
                            let output_content = match output.as_str() {
                                "json" => serde_json::to_string_pretty(&event)
                                    .unwrap_or_else(|_| event.content.clone()),
                                _ => format!("{}: {}", event.source, event.content),
                            };

                            println!("\n{}", output_content);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read staging directory: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
