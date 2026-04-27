use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;

mod daemon;
mod executor;
mod ingress;
mod ledger;
mod reflection;

use daemon::{EventPayload, MirrorDaemon};
use ingress::sanitizer::Sanitizer;

const MAX_EVENT_RETRIES: u8 = 3;
const RETRY_DELAY_SECS: u64 = 2;

/// Mirror - A witness daemon for deterministic data pipelines
#[derive(Parser)]
#[command(name = "mirror")]
#[command(about = "Witness and seal data pipeline executions", long_about = None)]
struct Cli {
    /// Path to the ledger directory
    #[arg(short, long, default_value = "./mirror-ledger")]
    ledger: PathBuf,

    /// Path to the pipelines directory
    #[arg(short, long, default_value = "./pipelines")]
    pipelines: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the daemon
    Run,
    /// Watch and ingest recordings
    Watch,
    /// Show statistics
    Stats,
    /// List available pipelines
    ListPipelines,
    /// Cleanup processed files in watch directory
    Cleanup,
    /// Diagnose issues
    Doctor,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run => {
            let daemon = MirrorDaemon::new(cli.ledger.clone(), cli.pipelines.clone())?;
            let (tx, rx) = tokio::sync::mpsc::channel::<EventPayload>(32);
            // Drop the sender immediately so the daemon loop exits cleanly when
            // no producers are attached (useful for a bare `run` invocation).
            drop(tx);
            daemon.run_async(rx).await?;
            Ok(())
        }
        Commands::ListPipelines => {
            let daemon = MirrorDaemon::new(cli.ledger.clone(), cli.pipelines.clone())?;
            let pipelines = daemon.list_pipelines()?;
            if pipelines.is_empty() {
                println!("No pipelines found in {}", cli.pipelines.display());
            } else {
                println!("Available pipelines:");
                for p in pipelines {
                    println!("  - {}", p);
                }
            }
            Ok(())
        }
        Commands::Watch => {
            let config_path = cli.pipelines.join("ingress.toml");
            ingress_watch(&config_path).await
        }
        Commands::Stats => {
            let ledger = ledger::Ledger::new(&cli.ledger)?;
            let all_entries = ledger.read_all()?;
            println!("Ledger statistics:");
            println!();
            println!("  Total reflections: {}", all_entries.len());
            println!(
                "  Successful: {}",
                all_entries.iter().filter(|e| e.success).count()
            );
            println!(
                "  Failed: {}",
                all_entries.len() - all_entries.iter().filter(|e| e.success).count()
            );
            if !all_entries.is_empty() {
                println!();
                println!(
                    "  First: {}",
                    all_entries.first().unwrap().ledger_time.to_rfc3339()
                );
                println!(
                    "  Last: {}",
                    all_entries.last().unwrap().ledger_time.to_rfc3339()
                );

                println!("\nRecent reflections:");
                let daemon = MirrorDaemon::new(cli.ledger.clone(), cli.pipelines.clone())?;
                for entry in daemon.list_recent(5)? {
                    let status = if entry.success { "✅" } else { "❌" };
                    println!(
                        "  {} {} ({})",
                        status,
                        entry.ledger_time.to_rfc3339(),
                        entry.pipeline
                    );
                }
            }
            Ok(())
        }
        Commands::Cleanup => {
            let config_path = cli.pipelines.join("ingress.toml");
            use ingress::config::Config;
            let config = Config::load(&config_path)?;

            // In a real implementation, we would iterate through the watch directory,
            // check if files are processed via the Watcher/Daemon state, and delete them.
            // For now, we'll just simulate the trigger for this task.
            println!(
                "Cleanup: Scanning {} for processed recordings...",
                config.capture.watch_dir.display()
            );
            println!("No cleanup action performed yet (feature pending implementation).");
            Ok(())
        }
        Commands::Doctor => {
            let config_path = cli.pipelines.join("ingress.toml");
            use ingress::config::Config;
            let config = Config::load(&config_path)?;
            ingress::doctor::run(&config).map_err(|e| anyhow::anyhow!(e))?;

            let ledger = ledger::Ledger::new(&cli.ledger)?;
            let all_entries = ledger.read_all()?;
            println!("Ledger diagnostics:");
            println!();
            if all_entries.is_empty() {
                println!("  No entries found in ledger.");
            } else {
                println!("  Total entries: {}", all_entries.len());
                println!(
                    "  Success rate: {:.2}%",
                    all_entries.iter().filter(|e| e.success).count() as f64
                        / all_entries.len() as f64
                        * 100.0
                );
            }
            Ok(())
        }
    }
}

/// Run the ingress watch loop: poll a directory for recordings and ingest them.
async fn ingress_watch(config_path: &std::path::Path) -> Result<(), anyhow::Error> {
    use ingress::{config::Config, db::Db, processor, transcription};
    use std::sync::Arc;

    use tracing::error;

    let config = Config::load(config_path)?;

    info!("mirror-daemon ingress starting up");
    info!("Watch dir:  {}", config.capture.watch_dir.display());
    info!("Database:   {}", config.storage.db_path.display());
    info!("Chunks dir: {}", config.storage.chunks_dir.display());
    info!(
        "Retention:  {}d fine / {}d coarse",
        config.retention.fine_grain_days, config.retention.coarse_grain_days
    );

    let db = Db::open(&config.storage.db_path)?;

    match db.oversaturation_report(5) {
        Ok(report) if !report.is_empty() => {
            info!("--- Oversaturation report (top recurring windows) ---");
            for (title, count) in &report {
                info!("  {:>4}x  {}", count, title);
            }
            info!("--- These are your gaps. They'll surface at distillation. ---");
        }
        Ok(_) => {}
        Err(e) => tracing::error!("Oversaturation report failed: {}", e),
    }

    info!("Ready. Waiting for new recordings... Press Ctrl+C to exit.");

    // 1. Initialize Message Bus
    let (tx, mut rx) = tokio::sync::mpsc::channel::<EventPayload>(32);

    // 2. Initialize Sources
    let sources: Vec<Box<dyn ingress::watcher::EventSource>> = {
        #[cfg(feature = "clipboard")]
        let mut sources: Vec<Box<dyn ingress::watcher::EventSource>> =
            vec![Box::new(ingress::watcher::FileWatcher::new(
                config.capture.watch_dir.clone(),
                config.capture.extensions.clone(),
                tx.clone(),
            ))];

        #[cfg(not(feature = "clipboard"))]
        let sources: Vec<Box<dyn ingress::watcher::EventSource>> =
            vec![Box::new(ingress::watcher::FileWatcher::new(
                config.capture.watch_dir.clone(),
                config.capture.extensions.clone(),
                tx.clone(),
            ))];

        #[cfg(feature = "clipboard")]
        sources.push(Box::new(ingress::clipboard_watcher::ClipboardWatcher::new(
            tx.clone(),
        )));

        sources
    };

    // 3. Start the Producers (Source Pollers) Task
    let sources = Arc::new(sources);
    let sources_handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            for source in sources.iter() {
                if let Err(e) = source.poll().await {
                    error!("Error running {} poll: {}", source.name(), e);
                }
            }
        }
    });

    // 4. Run the Consumer Task
    info!("Starting daemon consumer loop...");
    while let Some(event) = rx.recv().await {
        info!(
            "--- [Consumer] Received Event for Pipeline '{}' ---",
            &event.pipeline
        );

        let recording_path = std::path::PathBuf::from(&event.payload);

        match processor::process_recording(
            &recording_path,
            &config.processing,
            &config.storage.chunks_dir,
        )
        .await
        {
            Ok(result) => {
                info!(
                    "Processed {} -> {} chunks",
                    result.source_file,
                    result.chunks.len()
                );

                let sanitizer = Sanitizer::new()?;
                let recording_group_id = db.ensure_recording_group(&result.source_file)?;

                for chunk in &result.chunks {
                    let (id, inserted) = db.insert_or_get_chunk(chunk)?;
                    let hour_group_id = db.ensure_hour_group(chunk.started_at)?;
                    db.add_chunk_to_group(recording_group_id, id, Some(chunk.chunk_index))?;
                    db.add_chunk_to_group(hour_group_id, id, Some(chunk.chunk_index))?;
                    if let Some(window_title) = chunk.window_title.as_deref()
                        && !window_title.trim().is_empty()
                    {
                        let window_group_id = db.ensure_window_group(window_title)?;
                        db.add_chunk_to_group(window_group_id, id, Some(chunk.chunk_index))?;
                    }

                    if inserted {
                        info!(
                            "  Chunk #{} stored (id={}, {:.1}s)",
                            chunk.chunk_index, id, chunk.duration_secs,
                        );
                    } else {
                        info!(
                            "  Chunk #{} already stored (id={}), resuming downstream work",
                            chunk.chunk_index, id
                        );
                    }

                    if config.transcription.enabled && db.transcript_missing(id)? {
                        match transcription::transcribe_chunk(
                            std::path::Path::new(&chunk.chunk_path),
                            &config.transcription,
                        ) {
                            Ok(Some(transcript)) => {
                                let sanitized = sanitizer.sanitize_text(&transcript);
                                info!("  Transcribed: {}", sanitized);
                                db.update_transcript(id, &sanitized)?;
                            }
                            Ok(None) => {
                                info!("  Chunk appears silent");
                            }
                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                }

                if let Some(summary) = db.recording_group_summary(&result.source_file)? {
                    info!(
                        "Recording group '{}' now tracks {} chunks",
                        summary.label, summary.chunk_count
                    );
                }

                if let Some(first_chunk) = result.chunks.first()
                    && let Some(summary) = db.hour_group_summary(first_chunk.started_at)?
                {
                    info!(
                        "Hour group '{}' now tracks {} chunks",
                        summary.label, summary.chunk_count
                    );
                }

                if let Some(first_window_title) = result
                    .chunks
                    .iter()
                    .filter_map(|chunk| chunk.window_title.as_deref())
                    .find(|title| !title.trim().is_empty())
                    && let Some(summary) = db.window_group_summary(first_window_title)?
                {
                    info!(
                        "Window group '{}' now tracks {} chunks",
                        summary.label, summary.chunk_count
                    );
                }

                // Notify if chunks are due for distillation review
                if let Ok(due) = db.chunks_due_for_distillation(config.retention.fine_grain_days)
                    && !due.is_empty()
                {
                    info!(
                        "--- {} chunks past {}d threshold, ready for distillation ---",
                        due.len(),
                        config.retention.fine_grain_days
                    );
                    // ... processing ...
                }
            }
            Err(e) => {
                error!(
                    "Failed to process recording {}: {}",
                    recording_path.display(),
                    e
                );

                if event.attempts < MAX_EVENT_RETRIES {
                    let retry_tx = tx.clone();
                    let mut retry_event = event.clone();
                    retry_event.attempts += 1;
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_secs(RETRY_DELAY_SECS)).await;
                        if let Err(send_err) = retry_tx.send(retry_event).await {
                            error!(
                                "Failed to requeue event after processing error: {}",
                                send_err
                            );
                        }
                    });
                } else {
                    error!(
                        "Giving up on {} after {} attempts",
                        recording_path.display(),
                        event.attempts
                    );
                }
            }
        }
    }

    sources_handle.await.ok();
    Ok(())
}
