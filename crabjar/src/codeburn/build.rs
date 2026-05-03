use clap::CommandFactory;
use clap_mangen::Man;

#[derive(clap::Parser)]
#[command(
    name = "codeburn",
    about = "AI coding token usage tracker",
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

#[derive(clap::Subcommand, Clone)]
enum PlanAction {
    /// Set a subscription plan
    Set { name: String },
    /// Reset to no plan
    Reset,
    /// Show current plan
    Show,
}

fn main() -> std::io::Result<()> {
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or(std::io::ErrorKind::NotFound)?);
    let cmd = Cli::command();
    let man = Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;
    std::fs::write(out_dir.join("codeburn.1"), buffer)?;
    Ok(())
}
