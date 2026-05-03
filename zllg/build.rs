use clap::CommandFactory;
use clap_mangen::Man;

#[derive(clap::Parser)]
#[command(name = "zllg", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Boot a Zellij IDE session for the current (or given) directory.
    Boot {
        /// Override the project-type layout name (e.g. "rust", "node").
        /// Auto-detected from cwd markers if omitted.
        #[arg(short, long)]
        layout: Option<String>,

        /// Directory to open. Defaults to the current working directory.
        #[arg(short, long)]
        dir: Option<std::path::PathBuf>,
    },

    /// Pop the focused Zellij pane out into a new WezTerm window.
    Popout {
        /// Name of the pane to pop out (matches `[[panes]] name` in config).
        /// Uses the focused pane's config entry if omitted.
        #[arg(short, long)]
        pane: Option<String>,

        /// Keep the original pane alive (duplicate instead of move).
        #[arg(short, long)]
        duplicate: bool,
    },

    /// Duplicate the focused pane into a new WezTerm window (keeps original).
    Duplicate {
        /// Name of the pane to duplicate.
        #[arg(short, long)]
        pane: Option<String>,
    },

    /// Toggle the visibility of a named pane.
    Toggle {
        /// Logical pane name as defined in config (e.g. "git", "files").
        name: String,
    },

    /// Move the focused pane to a WezTerm workspace (monitor).
    MoveTo {
        /// Workspace name (e.g. "monitor-2").
        workspace: String,

        /// Name of the pane to move.
        #[arg(short, long)]
        pane: Option<String>,
    },

    /// Detect the project type in the current (or given) directory.
    Detect {
        /// Directory to inspect. Defaults to cwd.
        #[arg(short, long)]
        dir: Option<std::path::PathBuf>,
    },

    /// Check that required and optional tools are installed.
    Check,

    /// Scaffold config files and layouts on a fresh machine.
    Init {
        /// Overwrite existing files.
        #[arg(long)]
        force: bool,
    },

    /// Print the path to the active config file.
    ConfigPath,

    /// Print the IDE dashboard state.
    Dashboard {
        /// Override the project type for display.
        #[arg(short, long)]
        layout: Option<String>,
    },

    /// Scaffold keybinds for Zellij.
    Keybinds {
        /// Overwrite existing keybinds.
        #[arg(long)]
        force: bool,
    },

    /// Scaffold workspace config for WezTerm monitors.
    Workspaces {
        /// Overwrite existing workspaces.
        #[arg(long)]
        force: bool,
    },

    /// List available workspaces.
    ListWorkspaces,

    /// Run the IDE dashboard TUI (alternate screen).
    DashboardTui,

    /// Scaffold project-aware layouts.
    ProjectLayouts {
        /// Overwrite existing project layouts.
        #[arg(long)]
        force: bool,
    },
}

fn main() -> std::io::Result<()> {
    let out_dir =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or(std::io::ErrorKind::NotFound)?);
    let cmd = Cli::command();
    let man = Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;
    std::fs::write(out_dir.join("zllg.1"), buffer)?;
    Ok(())
}
