//! CLI module for a-hole command-line interface.
//!
//! This module provides the command-line interface for the a-hole config observer,
//! including argument parsing and command execution.

use crate::db::Database;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{debug, info};

/// Main CLI structure with argument parsing.
#[derive(Parser)]
#[command(name = "a-hole")]
#[command(about = "A Pi-hole for developer attention — mirrors telemetry and tracks config diffs as earned knowledge", long_about = None)]
pub struct Cli {
    /// Path to the SQLite database (default: ~/.config/a-hole/observer.db)
    #[arg(short, long, global = true)]
    pub db_path: Option<PathBuf>,

    /// Watch default config files (wezterm, zellij, nushell, zed)
    #[arg(short, long, default_value_t = false)]
    pub watch_defaults: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Subcommands for the a-hole CLI.
#[derive(Subcommand)]
pub enum Commands {
    /// Initialize the database and register watched files
    Init {
        /// Path to a config file to watch (can be specified multiple times)
        #[arg(value_name = "PATH", num_args = 0..)]
        paths: Option<Vec<String>>,
    },

    /// Start watching for config changes in foreground mode
    Watch {
        /// Watch default config files even if --paths not specified
        #[arg(short, long, default_value_t = false)]
        defaults: bool,
    },

    /// Show recent config changes from the database
    Log {
        /// Maximum number of changes to display
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Filter by tool name (e.g., wezterm, nushell)
        #[arg(short, long)]
        tool: Option<String>,

        /// Output as JSON instead of table format
        #[arg(short, long, default_value_t = false)]
        json: bool,
    },

    /// Show details for a specific change by ID
    Show {
        /// Change ID to display
        id: i64,

        /// Output as JSON instead of formatted output
        #[arg(short, long, default_value_t = false)]
        json: bool,
    },

    /// Revert a recorded change (restores previous content)
    Revert {
        /// Change ID to revert
        id: i64,

        /// Force revert even if file has diverged
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },

    /// Export change history as Markdown report
    Export {
        /// Output format (md or json)
        #[arg(short, long, default_value = "md")]
        format: String,

        /// Maximum number of changes to export
        #[arg(short, long, default_value = "100")]
        limit: usize,

        /// Output file path (- for stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// List currently watched files
    List {
        /// Show full details including status
        #[arg(short, long, default_value_t = false)]
        verbose: bool,
    },
}

/// Display format for changes in CLI output.
#[derive(Debug, Clone)]
pub struct ChangeDisplay {
    pub id: i64,
    pub timestamp: std::time::SystemTime,
    pub tool: String,
    pub file_path: String,
    pub change_kind: String,
    pub diff_format: String,
    pub summary_json: String,
}

impl Cli {
    /// Execute the CLI command.
    pub fn run(self) -> Result<()> {
        // Initialize tracing
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive("a-hole=debug".parse().unwrap()),
            )
            .init();

        debug!("CLI command parsed: {:?}", self.command);

        // Open database
        let db = match Database::new(self.db_path) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("Failed to open database: {}", e);
                return Err(anyhow::anyhow!("Database initialization failed"));
            }
        };

        debug!("Database opened at: {:?}", db.db_path());

        // Dispatch command
        match self.command {
            Commands::Init { paths } => Self::cmd_init(&db, paths),
            Commands::Watch { defaults } => Self::cmd_watch(&db, defaults || self.watch_defaults),
            Commands::Log { limit, tool, json } => Self::cmd_log(&db, limit, tool.as_deref(), json),
            Commands::Show { id, json } => Self::cmd_show(&db, id, json),
            Commands::Revert { id, force } => Self::cmd_revert(&db, id, force),
            Commands::Export {
                format,
                limit,
                output,
            } => Self::cmd_export(&db, &format, limit, output.as_deref()),
            Commands::List { verbose } => Self::cmd_list(&db, verbose),
        }
    }

    /// Handle init command - creates DB and registers watched files.
    fn cmd_init(db: &Database, paths: Option<Vec<String>>) -> Result<()> {
        info!("Initializing a-hole observer");

        println!("✓ Database initialized at: {}", db.db_path().display());

        // Register default watched files from MVP spec
        let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
        let defaults = vec![
            format!("{}/.config/wezterm/wezterm.lua", home),
            format!("{}/.config/zellij/config.kdl", home),
            format!("{}/.config/nushell/config.nu", home),
            format!("{}/.config/nushell/env.nu", home),
            format!("{}/.config/zed/settings.json", home),
        ];

        let mut registered = 0usize;
        for path in &defaults {
            match std::fs::canonicalize(path) {
                Ok(normalized) => {
                    if let Some(id) = db
                        .register_watched_file(path, &normalized, "unknown", "other")
                        .ok()
                    {
                        // Try to infer tool from path
                        let tool = Self::infer_tool_from_path(&PathBuf::from(path));
                        let _ = db.update_watched_file_status(id, "Active");
                        registered += 1;
                    }
                }
                Err(_) => {
                    // Register as pending - user may create it later
                    if let Ok(normalized) = std::path::PathBuf::from(path).canonicalize() {
                        let _ = db.register_watched_file(path, &normalized, "unknown", "other");
                    } else {
                        registered += 1; // Count even pending files
                    }
                }
            }
        }

        println!("✓ Registered {} default watched files", registered);
        println!("\nDefault watch targets:");
        for path in &defaults {
            println!("  - {}", path);
        }

        if let Some(custom_paths) = paths {
            for path_str in custom_paths {
                match std::path::PathBuf::from(&path_str).canonicalize() {
                    Ok(normalized) => {
                        let tool = Self::infer_tool_from_path(&normalized);
                        db.register_watched_file(&path_str, &normalized, &tool, "other")?;
                        println!("✓ Registered custom watch: {}", path_str);
                    }
                    Err(e) => {
                        eprintln!("  ⚠ Could not resolve {}: {}", path_str, e);
                    }
                }
            }
        }

        println!("\nUse `a-hole watch` to start observing changes.");
        println!("Use `a-hole log` to view recorded changes.");
        Ok(())
    }

    /// Handle watch command - starts foreground watcher.
    fn cmd_watch(db: &Database, watch_defaults: bool) -> Result<()> {
        info!("Starting watch command");

        let config = crate::observer::WatcherConfig {
            debounce_duration: std::time::Duration::from_millis(500),
            watch_defaults,
        };

        // Wrap database for Arc/Mutex usage by watcher
        use std::sync::{Arc, Mutex};
        let db_arc = Arc::new(Mutex::new(db.clone()));

        match crate::observer::ConfigWatcher::start(&db_arc, config) {
            Ok(_) => {
                info!("Watch loop completed normally");
                println!("\nWatcher stopped.");
                Ok(())
            }
            Err(e) => {
                tracing::error!("Watcher failed: {}", e);
                eprintln!("Error starting watcher: {}", e);
                std::process::exit(1);
            }
        }
    }

    /// Handle log command - displays recent changes.
    fn cmd_log(db: &Database, limit: usize, tool: Option<&str>, json: bool) -> Result<()> {
        info!("Fetching {} config changes", limit);

        let changes = if let Some(t) = tool {
            db.get_config_changes_for_file(0)? // Simplified - in real impl would filter by tool
        } else {
            db.get_config_changes(Some(limit))?
        };

        if changes.is_empty() {
            println!("No config changes recorded yet.");
            println!("\nTo start tracking:");
            println!("  1. Run `a-hole init` to register files you want to watch");
            println!("  2. Run `a-hole watch` to start observing changes");
            return Ok(());
        }

        if json {
            // JSON output for scripting/Nushell compatibility
            let display_changes: Vec<ChangeDisplay> = changes
                .iter()
                .map(|c| ChangeDisplay {
                    id: c.id,
                    timestamp: c.timestamp,
                    tool: "unknown".to_string(), // Would need to join with watched_files
                    file_path: format!("id={}", c.watched_file_id),
                    change_kind: c.change_kind.clone(),
                    diff_format: c.diff_format.clone(),
                    summary_json: c.summary_json.clone(),
                })
                .collect();

            println!("{}", serde_json::to_string_pretty(&display_changes)?);
        } else {
            // Human-readable table format
            println!("\n=== Recent Config Changes ===\n");

            for change in &changes[..limit.min(changes.len())] {
                let summary: crate::domain::DiffSummary =
                    serde_json::from_str(&change.summary_json).unwrap_or_default();

                println!(
                    "Change #{} | {} | {}",
                    change.id,
                    change.change_kind.to_uppercase(),
                    change.timestamp.format("%Y-%m-%d %H:%M:%S")
                );
                println!(
                    "  Format: {} | Lines changed: {} (+{}, -{})",
                    change.diff_format,
                    summary.total_changes,
                    summary.lines_added,
                    summary.lines_removed
                );
            }

            if changes.len() > limit {
                println!(
                    "... and {} more changes. Use `a-hole log --limit N` to see more.",
                    changes.len() - limit
                );
            }
        }

        Ok(())
    }

    /// Handle show command - displays details for a single change.
    fn cmd_show(db: &Database, id: i64, json: bool) -> Result<()> {
        info!("Fetching change #{}", id);

        let change = db.get_config_change(id)?;

        match change {
            Some(change) => {
                if json {
                    // JSON output
                    let summary: crate::domain::DiffSummary =
                        serde_json::from_str(&change.summary_json).unwrap_or_default();
                    let json_output = serde_json::json!({
                        "id": change.id,
                        "watched_file_id": change.watched_file_id,
                        "previous_snapshot_id": change.previous_snapshot_id,
                        "current_snapshot_id": change.current_snapshot_id,
                        "timestamp": change.timestamp.format("%Y-%m-%d %H:%M:%S"),
                        "change_kind": change.change_kind,
                        "diff_format": change.diff_format,
                        "summary": summary,
                    });
                    match serde_json::to_string_pretty(&json_output) {
                        Ok(s) => println!("{}", s),
                        Err(e) => eprintln!("Failed to serialize JSON: {}", e),
                    }
                } else {
                    // Formatted output
                    let summary: crate::domain::DiffSummary =
                        serde_json::from_str(&change.summary_json).unwrap_or_default();

                    println!("\n=== Change #{} ===\n", change.id);
                    println!("Watched File ID: {}", change.watched_file_id);
                    println!("Change Kind: {}", change.change_kind.to_uppercase());
                    println!(
                        "Timestamp: {}",
                        change.timestamp.format("%Y-%m-%d %H:%M:%S")
                    );
                    println!("\nDiff Summary:");
                    println!("  Format: {}", change.diff_format);
                    println!("  Total Changes: {}", summary.total_changes);
                    println!("  Lines Added: {}", summary.lines_added);
                    println!("  Lines Removed: {}", summary.lines_removed);

                    if !summary.keys_changed.is_empty() {
                        println!("\nKeys Changed:");
                        for key in &summary.keys_changed {
                            println!("  - {}", key);
                        }
                    }

                    // Show previous and current snapshot contents if available
                    let prev_content =
                        db.get_snapshot_content(change.previous_snapshot_id.unwrap_or(0))?;
                    let curr_content = db.get_snapshot_content(change.current_snapshot_id)?;

                    if let Some(prev) = &prev_content {
                        println!("\n--- Previous Content (truncated) ---");
                        for line in prev.lines().take(10) {
                            println!("  {}", line);
                        }
                    }

                    if let Some(curr) = &curr_content {
                        println!("\n--- Current Content (truncated) ---");
                        for line in curr.lines().take(10) {
                            println!("  {}", line);
                        }
                    }

                    println!();
                }

                Ok(())
            }
            None => {
                eprintln!("Change #{} not found", id);
                std::process::exit(1);
            }
        }
    }

    /// Handle revert command - restores previous content for a change.
    fn cmd_revert(db: &Database, id: i64, force: bool) -> Result<()> {
        info!("Attempting to revert change #{}", id);

        let change = db.get_config_change(id)?;

        match change {
            Some(change) => {
                // Check if we have a previous snapshot to restore from
                let prev_snapshot_id = match change.previous_snapshot_id {
                    Some(id) => id,
                    None => {
                        eprintln!("Change #{} has no prior state to revert to", id);
                        std::process::exit(1);
                    }
                };

                // Get the previous content
                let prev_content = db.get_snapshot_content(prev_snapshot_id)?;

                match prev_content {
                    Some(content) => {
                        // Determine which file was changed (need to join with watched_files)
                        let watched_file = db.get_watched_file(change.watched_file_id)?;

                        if let Some(file) = watched_file {
                            // Safety check: verify current state matches expected post-change state
                            if !force {
                                if let Ok(current_content) =
                                    std::fs::read_to_string(&file.normalized_path)
                                {
                                    let curr_snapshot =
                                        db.get_snapshot(change.current_snapshot_id)?;
                                    if let Some(snapshot) = curr_snapshot {
                                        if content != current_content
                                            && snapshot.content != current_content
                                        {
                                            eprintln!(
                                                "⚠ File has diverged since change #{} was recorded",
                                                id
                                            );
                                            eprintln!(
                                                "  Current: {} bytes, Expected: {} bytes",
                                                current_content.len(),
                                                snapshot.content.len()
                                            );

                                            if !force {
                                                println!("\nUse --force to overwrite anyway (may cause data loss).");
                                                std::process::exit(1);
                                            }
                                        }
                                    }
                                } else {
                                    eprintln!("⚠ Cannot read current file state, using force mode");
                                }
                            }

                            // Write previous content back to file
                            match std::fs::write(&file.normalized_path, &content) {
                                Ok(_) => {
                                    info!("Successfully reverted change #{}", id);

                                    // Record the revert operation
                                    let _ =
                                        db.record_revert(id, change.watched_file_id, true, None);

                                    println!("✓ Change #{} successfully reverted", id);
                                    println!("  Restored: {}", file.normalized_path.display());
                                }
                                Err(e) => {
                                    tracing::error!("Failed to write reverted content: {}", e);
                                    let _ = db.record_revert(
                                        id,
                                        change.watched_file_id,
                                        false,
                                        Some(format!("{}", e)),
                                    );
                                    eprintln!("✗ Failed to revert: {}", e);
                                    std::process::exit(1);
                                }
                            }

                            Ok(())
                        } else {
                            eprintln!(
                                "Watched file #{} not found in database",
                                change.watched_file_id
                            );
                            std::process::exit(1);
                        }
                    }
                    None => {
                        eprintln!("No previous snapshot found for change #{}", id);
                        std::process::exit(1);
                    }
                }
            }
            None => {
                eprintln!("Change #{} not found", id);
                std::process::exit(1);
            }
        }
    }

    /// Handle export command - exports changes as Markdown or JSON.
    fn cmd_export(
        db: &Database,
        format: &str,
        limit: usize,
        output: Option<&std::path::Path>,
    ) -> Result<()> {
        info!("Exporting {} changes in {:?} format", limit, format);

        let changes = db.export_changes_with_details(Some(limit))?;

        let content = match format.to_lowercase().as_str() {
            "json" => Self::export_json(&changes)?,
            "md" | "markdown" => Self::export_markdown(&changes),
            _ => {
                eprintln!("Unsupported export format: {}. Use 'md' or 'json'.", format);
                std::process::exit(1);
            }
        };

        // Write to file or stdout
        match output {
            Some(path) if path != &std::path::Path::new("-") => {
                std::fs::write(path, &content)?;
                println!("Export written to: {}", path.display());
            }
            _ => {
                print!("{}", content);
            }
        }

        Ok(())
    }

    /// Handle list command - shows watched files.
    fn cmd_list(db: &Database, verbose: bool) -> Result<()> {
        info!("Listing watched files");

        let files = db.list_watched_files_display()?;

        if files.is_empty() {
            println!("No watched files registered.");
            println!("\nUse `a-hole init` to register files you want to watch.");
            return Ok(());
        }

        if verbose {
            // Full details format
            for file in &files {
                let status_icon = match file.status.as_str() {
                    "Active" => "✓",
                    "Pending" => "?",
                    _ => "✗",
                };

                println!(
                    "{} {} ({})",
                    status_icon,
                    file.normalized_path.display(),
                    file.tool
                );
                println!(
                    "  ID: {} | Type: {} | Status: {}",
                    file.id, file.file_type, file.status
                );
            }
        } else {
            // Summary format
            println!("\n=== Watched Files ===\n");

            for file in &files {
                let status_icon = match file.status.as_str() {
                    "Active" => "✓",
                    "Pending" => "?",
                    _ => "✗",
                };

                println!(
                    "{} {} [{}]",
                    status_icon,
                    file.normalized_path.display(),
                    file.tool
                );
            }
        }

        Ok(())
    }

    /// Helper to infer tool name from path.
    fn infer_tool_from_path(path: &std::path::PathBuf) -> String {
        let path_str = path.to_string_lossy().to_lowercase();

        if path_str.contains("wezterm") {
            "wezterm"
        } else if path_str.contains("zellij") {
            "zellij"
        } else if path_str.contains("nushell") || path_str.ends_with(".nu") {
            "nushell"
        } else if path_str.contains("zed") {
            "zed"
        } else if path_str.contains("helix") {
            "helix"
        } else {
            "unknown"
        }
        .to_string()
    }

    /// Export changes as Markdown report.
    fn export_markdown(changes: &[db::ChangeExport]) -> String {
        use std::fmt::Write;

        let mut output = String::new();

        writeln!(output, "# Config Change History\n").unwrap();
        writeln!(output, "Generated by a-hole config observer.\n").unwrap();

        if changes.is_empty() {
            writeln!(output, "No changes recorded yet.\n").unwrap();
            return output;
        }

        for change in changes {
            let summary: crate::domain::DiffSummary =
                serde_json::from_str(&change.summary_json).unwrap_or_default();

            writeln!(
                output,
                "## Change #{} - {} ({})",
                change.id, change.tool, change.watched_path
            )
            .unwrap();
            writeln!(
                output,
                "\n**Timestamp:** {}\n",
                change.timestamp.format("%Y-%m-%d %H:%M:%S")
            )
            .unwrap();
            writeln!(output, "**Tool:** {}\n", change.tool).unwrap();
            writeln!(output, "**File:** {}\n", change.watched_path).unwrap();
            writeln!(
                output,
                "**Change Type:** {}\n",
                change.change_kind.to_uppercase()
            )
            .unwrap();

            writeln!(output, "### Diff Summary\n").unwrap();
            writeln!(output, "- **Format:** {}\n", change.diff_format).unwrap();
            writeln!(output, "- **Total Changes:** {}\n", summary.total_changes).unwrap();
            writeln!(output, "- **Lines Added:** {}\n", summary.lines_added).unwrap();
            writeln!(output, "- **Lines Removed:** {}\n", summary.lines_removed).unwrap();

            if !summary.keys_changed.is_empty() {
                writeln!(output, "### Keys Changed\n").unwrap();
                for key in &summary.keys_changed {
                    writeln!(output, "- `{}`\n", key).unwrap();
                }
            }

            if let Some(prev) = &change.previous_content {
                writeln!(output, "### Previous Content (truncated)\n```diff\n").unwrap();
                for line in prev.lines().take(20) {
                    writeln!(output, "- {}", line).unwrap();
                }
                writeln!(output, "```\n").unwrap();
            }

            if let Some(curr) = &change.current_content {
                writeln!(output, "### Current Content (truncated)\n```diff\n").unwrap();
                for line in curr.lines().take(20) {
                    writeln!(output, "+ {}", line).unwrap();
                }
                writeln!(output, "```\n").unwrap();
            }

            writeln!(output, "---\n").unwrap();
        }

        output
    }

    /// Export changes as JSON array.
    fn export_json(changes: &[db::ChangeExport]) -> Result<String> {
        let data: Vec<serde_json::Value> = changes.iter().map(|c| {
            serde_json::json!({
                "id": c.id,
                "timestamp": c.timestamp.format("%Y-%m-%d %H:%M:%S"),
                "tool": c.tool,
                "file_path": c.watched_path,
                "change_kind": c.change_kind,
                "diff_format": c.diff_format,
                "summary": serde_json::from_str::<serde_json::Value>(&c.summary_json).unwrap_or_default(),
            })
        }).collect();

        Ok(serde_json::to_string_pretty(&data)?)
    }
}
