//! A-hole: Local config observer for terminal-centric developers.
//!
//! This is the Pi-hole moment for config management - watch what users actually touch,
//! record real changes, and make them inspectable and reversible from a truthful CLI.
//!
//! # Core Principles
//! - Observe first, declare never
//! - The change is the product, not the full config file
//! - Mirror locally, do not block or replace upstream tooling
//! - Tell the truth about what the software is doing

mod capture;
mod cli;
mod db;
mod diff;
mod domain;
mod observer;

use anyhow::Result;
use std::path::PathBuf;
use tracing::{debug, error, info};
use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    let cli = cli::Cli::parse();

    // Initialize tracing
    fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("a-hole=debug".parse().unwrap()),
        )
        .init();

    if let Err(e) = cli.run() {
        eprintln!("Error: {}", e);

        // Categorize and present errors truthfully per MVP spec
        let error_str = format!("{}", e);

        if error_str.contains("Database") || error_str.contains("init") {
            eprintln!("\nThis may require running `a-hole init` first.");
        } else if error_str.contains("not found") {
            eprintln!("\nThe specified resource does not exist or is not accessible.");
        }

        std::process::exit(1);
    }
}
