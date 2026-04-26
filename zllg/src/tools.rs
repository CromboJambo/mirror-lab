use anyhow::Result;

/// Check whether a tool is available on `$PATH`.
pub fn is_available(name: &str) -> bool {
    which::which(name).is_ok()
}

/// Print a formatted report of required and optional tools.
pub fn check_tools() -> Result<()> {
    let required: &[(&str, &str)] = &[
        ("zellij", "terminal multiplexer — the IDE substrate"),
        (
            "wezterm",
            "terminal emulator — pop-out and workspace support",
        ),
    ];

    let optional: &[(&str, &str)] = &[
        ("helix", "Helix editor — primary editor pane"),
        ("yazi", "file manager — file tree pane"),
        ("lazygit", "git TUI — git pane"),
        ("nu", "Nushell — shell pane and scripting layer"),
        ("zsh", "POSIX fallback shell"),
        ("cargo-watch", "Rust watcher — watch pane for Rust projects"),
    ];

    println!("zllg tool check\n");

    let mut all_required_ok = true;

    println!("Required:");
    for (name, desc) in required {
        let ok = is_available(name);
        if !ok {
            all_required_ok = false;
        }
        let mark = if ok { "✓" } else { "✗" };
        println!("  {mark} {name:<15} {desc}");
    }

    println!("\nOptional:");
    for (name, desc) in optional {
        let ok = is_available(name);
        let mark = if ok { "✓" } else { "·" };
        println!("  {mark} {name:<15} {desc}");
    }

    if !all_required_ok {
        println!("\nwarn: one or more required tools are missing.");
    } else {
        println!("\nAll required tools found.");
    }

    Ok(())
}
