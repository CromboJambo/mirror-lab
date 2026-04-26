use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::config::load_config;
use crate::{
    config, dashboard, dashboard_tui, detect, keybind, layout, pane, project_layout, tools,
    workspace,
};

/// zllg — Zellij IDE orchestration layer (zellige)
///
/// Each pane is a TUI app locked in position in a Zellij KDL layout.
/// Use `zllg boot` to start a project-aware IDE session, `zllg popout`
/// to float a pane into its own WezTerm window, and `zllg init` to
/// scaffold the config on a fresh machine.
///
/// Subcommands:
///   boot          Start a Zellij IDE session
///   detect        Detect project type in a directory
///   check         Verify required tools are installed
///   init          Scaffold all config files
///   popout        Float a pane into a new WezTerm window
///   duplicate     Duplicate a pane into a new window
///   toggle        Toggle pane visibility
///   move_to       Move a pane to a WezTerm workspace
///   dashboard     Print the IDE dashboard state
///   dashboard_tui Run the IDE dashboard TUI
///   keybinds      Scaffold Zellij keybinds
///   workspaces    Scaffold WezTerm workspace config
///   project_layouts Scaffold project-aware layouts
///   config_path   Print the config file path
///   list_workspaces List available workspaces
#[derive(Debug, Parser)]
#[command(name = "zllg", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Boot a Zellij IDE session for the current (or given) directory.
    Boot {
        /// Override the project-type layout name (e.g. "rust", "node").
        /// Auto-detected from cwd markers if omitted.
        #[arg(short, long)]
        layout: Option<String>,

        /// Directory to open. Defaults to the current working directory.
        #[arg(short, long)]
        dir: Option<PathBuf>,
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
        dir: Option<PathBuf>,
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

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            Commands::Boot { layout, dir } => {
                let cwd = resolve_dir(dir)?;
                let layout_name = layout.unwrap_or_else(|| {
                    let pt = detect::detect_project_type(&cwd);
                    println!("detected project type: {pt}");
                    pt.to_string()
                });

                let layout_path = layout::resolve_layout(&layout_name)?;
                println!("booting zllg with layout: {}", layout_path.display());

                let status = std::process::Command::new("zellij")
                    .arg("--layout")
                    .arg(&layout_path)
                    .current_dir(&cwd)
                    .status()?;

                if !status.success() {
                    anyhow::bail!("zellij exited with status {}", status);
                }
            }

            Commands::Popout { pane, duplicate } => {
                let cwd = std::env::current_dir()?;
                pane::popout(pane.as_deref(), duplicate, &cwd)?;
            }

            Commands::Duplicate { pane } => {
                let cwd = std::env::current_dir()?;
                pane::popout(pane.as_deref(), true, &cwd)?;
            }

            Commands::Toggle { name } => {
                pane::toggle_pane(&name)?;
            }

            Commands::MoveTo { workspace, pane } => {
                let cwd = std::env::current_dir()?;
                pane::move_to_workspace(&workspace, pane.as_deref(), &cwd)?;
            }

            Commands::Detect { dir } => {
                let cwd = resolve_dir(dir)?;
                let pt = detect::detect_project_type(&cwd);
                println!("{pt}");
            }

            Commands::Check => {
                tools::check_tools()?;
            }

            Commands::Init { force } => {
                println!("initialising zllg config...\n");

                let cfg_path = config::config_path();
                if !cfg_path.exists() || force {
                    let written = config::write_default_config()?;
                    println!("  wrote {}", written.display());
                } else {
                    println!("  exists {} (use --force to overwrite)", cfg_path.display());
                }

                println!("\nlayouts:");
                layout::write_bundled_layouts()?;

                println!("\nkeybinds:");
                keybind::write_default_keybinds()?;

                println!("\nworkspaces:");
                workspace::write_default_workspaces()?;

                println!("\nproject layouts:");
                project_layout::write_default_project_layouts()?;

                println!("\nDone. Run `zllg check` to verify your toolchain.");
            }

            Commands::ConfigPath => {
                println!("{}", config::config_path().display());
            }

            Commands::Dashboard { layout } => {
                let cwd = resolve_dir(None)?;
                let pt = layout
                    .map(|l| l.parse::<crate::detect::ProjectType>().unwrap())
                    .unwrap_or_else(|| detect::detect_project_type(&cwd));
                let cfg = load_config()?;
                let state = dashboard::build_dashboard(&cfg, pt);
                println!("{}", dashboard::render_dashboard(&state));
            }

            Commands::Keybinds { force } => {
                let kb_path = keybind::keybind_path();
                if !kb_path.exists() || force {
                    let written = keybind::write_default_keybinds()?;
                    println!("  wrote {}", written.display());
                } else {
                    println!("  exists {} (use --force to overwrite)", kb_path.display());
                }
                println!("\nrendered keybind KDL:");
                println!("{}", keybind::render_keybind_kdl());
            }

            Commands::Workspaces { force } => {
                let ws_path = workspace::workspace_path();
                if !ws_path.exists() || force {
                    let written = workspace::write_default_workspaces()?;
                    println!("  wrote {}", written.display());
                } else {
                    println!("  exists {} (use --force to overwrite)", ws_path.display());
                }
            }

            Commands::ListWorkspaces => {
                let cfg = workspace::load_workspaces()?;
                println!("zllg workspaces\n");
                for ws in &cfg.workspaces {
                    println!("  {name:<15} {label}", name = ws.name, label = ws.label);
                }
            }

            Commands::DashboardTui => {
                dashboard_tui::run_dashboard_tui()?;
            }

            Commands::ProjectLayouts { force } => {
                let pl_path = project_layout::project_layout_path();
                if !pl_path.exists() || force {
                    let written = project_layout::write_default_project_layouts()?;
                    println!("  wrote {}", written.display());
                } else {
                    println!("  exists {} (use --force to overwrite)", pl_path.display());
                }
            }
        }

        Ok(())
    }
}

fn resolve_dir(dir: Option<PathBuf>) -> Result<PathBuf> {
    match dir {
        Some(d) => Ok(d),
        None => Ok(std::env::current_dir()?),
    }
}
