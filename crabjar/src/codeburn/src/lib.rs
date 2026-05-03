use clap::{CommandFactory, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "codeburn",
    about = "AI coding token usage tracker",
    disable_help_flag = true,
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[derive(Debug, Subcommand, Clone)]
pub enum CliCommand {
    /// Show help as structured JSON
    Help,

    /// Report token usage for a period
    Report {
        #[arg(short, long)]
        period: Option<String>,

        #[arg(short = 's', long)]
        from: Option<String>,

        #[arg(short, long)]
        to: Option<String>,

        #[arg(short = 'r', long)]
        provider: Option<String>,

        #[arg(short = 'j', long)]
        project: Option<String>,

        #[arg(short = 'x', long)]
        exclude: Option<String>,

        #[arg(short, long, default_value = "json")]
        format: String,

        #[arg(short = 'e', long)]
        refresh: Option<u64>,
    },

    /// Compact one-liner status
    Status {
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Export CSV/JSON multi-period data
    Export {
        #[arg(short, long, default_value = "csv")]
        format: String,
    },

    /// Find token waste patterns
    Optimize {
        #[arg(short, long)]
        period: Option<String>,
    },

    /// Side-by-side model comparison
    Compare {
        #[arg(short, long)]
        period: Option<String>,
    },

    /// Set display currency
    Currency { code: String },

    /// Map provider model name to canonical pricing name
    ModelAlias { from: String, to: String },

    /// Subscription plan tracking
    Plan {
        #[command(subcommand)]
        action: PlanAction,
    },
}

#[derive(Debug, Subcommand, Clone)]
pub enum PlanAction {
    /// Set a subscription plan
    Set { name: String },
    /// Reset to no plan
    Reset,
    /// Show current plan
    Show,
}

pub fn cli() -> clap::Command {
    Cli::command()
}
