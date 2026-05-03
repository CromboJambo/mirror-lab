use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "crabjar",
    about = "CLI for local state-docs management",
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

    /// Manage state-docs
    State {
        #[command(subcommand)]
        command: StateCommand,
    },

    /// Manage knowledge store
    Knowledge {
        #[command(subcommand)]
        command: crate::knowledge_store::commands::KnowledgeCommand,
    },

    /// Manage dotfile promotions
    Dotfile {
        #[command(subcommand)]
        command: crate::dotfile_manager::DotfileCommand,
    },

    /// Show workspace configuration
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
}

#[derive(Debug, Subcommand, Clone)]
pub enum StateCommand {
    /// List all state-docs
    List,
    /// Show a state-doc with annotations
    Show { doc: String },
    /// Add a note annotation
    Annotate { doc: String, message: String },
    /// Add a question annotation
    Question { doc: String, message: String },
    /// Resolve an annotation
    Resolve { doc: String, id: String },
}

#[derive(Debug, Subcommand, Clone)]
pub enum WorkspaceCommand {
    /// Show workspace configuration status
    Status,
}

pub fn cli() -> clap::Command {
    Cli::command()
}
