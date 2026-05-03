use clap::CommandFactory;
use clap_mangen::Man;

#[derive(clap::Parser)]
#[command(
    name = "crabjar",
    about = "CLI for local state-docs management",
    disable_help_flag = true,
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(clap::Subcommand, Clone)]
enum CliCommand {
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

#[derive(clap::Subcommand, Clone)]
enum StateCommand {
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

#[derive(clap::Subcommand, Clone)]
enum WorkspaceCommand {
    /// Show workspace configuration status
    Status,
}

fn main() -> std::io::Result<()> {
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or(std::io::ErrorKind::NotFound)?);
    let cmd = Cli::command();
    let man = Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;
    std::fs::write(out_dir.join("crabjar.1"), buffer)?;
    Ok(())
}
