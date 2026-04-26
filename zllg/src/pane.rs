use anyhow::{Context, Result, bail};
use std::process::Command;

use crate::config::{PaneConfig, load_config};

/// Resolve the pane config entry by name.
pub fn find_pane<'a>(panes: &'a [PaneConfig], name: &str) -> Option<&'a PaneConfig> {
    panes.iter().find(|p| p.name == name)
}

/// Pop the focused Zellij pane into a new WezTerm window.
/// If `duplicate` is true, the original pane is kept alive; otherwise it is
/// ejected from the Zellij session.
pub fn popout(pane_name: Option<&str>, duplicate: bool, cwd: &std::path::Path) -> Result<()> {
    let cfg = load_config()?;

    // Determine the command to run in the new window.
    let (cmd, args) = if let Some(name) = pane_name {
        let pane = find_pane(&cfg.panes, name)
            .with_context(|| format!("no pane named '{}' in config", name))?;
        (pane.command.clone(), pane.args.clone())
    } else {
        // Fall back to the configured shell.
        (cfg.default_shell.clone(), vec![])
    };

    // Build the wezterm spawn command.
    let mut wt = Command::new("wezterm");
    wt.arg("cli")
        .arg("spawn")
        .arg("--new-window")
        .arg("--cwd")
        .arg(cwd);

    wt.arg("--").arg(&cmd).args(&args);

    let status = wt
        .status()
        .context("failed to execute `wezterm cli spawn`")?;

    if !status.success() {
        bail!("`wezterm cli spawn` exited with status {}", status);
    }

    // Hide the original pane unless duplicating.
    if !duplicate {
        eject_focused_pane()?;
    }

    Ok(())
}

/// Move the focused pane to a named WezTerm workspace (monitor).
pub fn move_to_workspace(
    workspace: &str,
    pane_name: Option<&str>,
    cwd: &std::path::Path,
) -> Result<()> {
    let cfg = load_config()?;

    let (cmd, args) = if let Some(name) = pane_name {
        let pane = find_pane(&cfg.panes, name)
            .with_context(|| format!("no pane named '{}' in config", name))?;
        (pane.command.clone(), pane.args.clone())
    } else {
        (cfg.default_shell.clone(), vec![])
    };

    let mut wt = Command::new("wezterm");
    wt.arg("cli")
        .arg("spawn")
        .arg("--new-window")
        .arg("--workspace")
        .arg(workspace)
        .arg("--cwd")
        .arg(cwd)
        .arg("--")
        .arg(&cmd)
        .args(&args);

    let status = wt
        .status()
        .context("failed to execute `wezterm cli spawn`")?;

    if !status.success() {
        bail!("`wezterm cli spawn` exited with status {}", status);
    }

    // Close the original Zellij pane.
    zellij_action(&["close-pane"])?;

    Ok(())
}

/// Toggle visibility of a named pane via `zellij action focus-pane-with-index`
/// followed by `zellij action toggle-pane-frames`.
pub fn toggle_pane(name: &str) -> Result<()> {
    let cfg = load_config()?;
    let pane = find_pane(&cfg.panes, name)
        .with_context(|| format!("no pane named '{}' in config", name))?;

    zellij_action(&["focus-pane-with-index", &pane.index.to_string()])?;
    zellij_action(&["toggle-pane-frames"])?;

    Ok(())
}

// ── Zellij helpers ──────────────────────────────────────────────────────────

fn zellij_action(args: &[&str]) -> Result<()> {
    let status = Command::new("zellij")
        .arg("action")
        .args(args)
        .status()
        .context("failed to execute `zellij action`")?;

    if !status.success() {
        bail!("`zellij action {}` exited with {}", args.join(" "), status);
    }
    Ok(())
}

/// Eject (toggle-embed-or-eject) the focused pane from the Zellij session.
fn eject_focused_pane() -> Result<()> {
    zellij_action(&["toggle-pane-embed-or-eject"])
}
