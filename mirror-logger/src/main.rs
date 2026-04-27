use clap::{Parser, Subcommand};

mod commands;
mod engine;
mod entry;
mod store;
mod tri;

#[derive(Parser)]
#[command(name = "mirror-log")]
#[command(about = "Tri-state decision system with memory and re-evaluation", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new entry to the log
    Add(commands::add::AddArgs),
    /// List entries
    List(commands::list::ListArgs),
    /// Revisit hold entries for re-evaluation
    Revisit(commands::revisit::RevisitArgs),
    /// Resolve an entry (create new entry referencing parent)
    Resolve(commands::resolve::ResolveArgs),
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Add(args) => {
            if let Err(e) = commands::add::add_entry(args) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::List(args) => {
            if let Err(e) = commands::list::list_entries(args) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Revisit(args) => {
            if let Err(e) = commands::revisit::revisit_entries(args) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Resolve(args) => {
            if let Err(e) = commands::resolve::resolve_entry(args) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}
