use anyhow::{Result, bail};
use std::path::PathBuf;

/// Resolve the layouts directory: `~/.config/zllg/layouts/`.
pub fn layouts_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("zllg")
        .join("layouts")
}

/// Find the KDL layout file for the given name.
/// Falls back to `default.kdl` if the specific one is absent.
pub fn resolve_layout(name: &str) -> Result<PathBuf> {
    let dir = layouts_dir();
    let specific = dir.join(format!("{name}.kdl"));
    if specific.exists() {
        return Ok(specific);
    }
    let fallback = dir.join("default.kdl");
    if fallback.exists() {
        eprintln!("warn: no layout for '{name}', falling back to default.kdl");
        return Ok(fallback);
    }
    bail!(
        "no layout file found for '{}' and no default.kdl in {}",
        name,
        dir.display()
    )
}

/// The KDL source for the default layout (shipped inline so `zllg init`
/// can write it out without requiring a separate data directory).
pub const DEFAULT_LAYOUT_KDL: &str = r#"layout {
    pane size=1 borderless=true {
        plugin location="zellij:tab-bar"
    }

    pane split_direction="vertical" {
        // Left: file tree — locked, hideable with Alt-f
        pane size="18%" name="files" {
            command "yazi"
        }

        // Centre: editor column
        pane split_direction="horizontal" name="editor_col" {
            pane size="70%" name="editor" {
                command "helix"
                args "."
            }
            // Bottom strip: shell + cargo watch
            pane split_direction="vertical" size="30%" {
                pane name="shell" {
                    command "nu"
                }
                pane name="watch" {
                    command "nu"
                    args "-c" "echo 'no watcher configured'"
                }
            }
        }

        // Right: git — locked, hideable with Alt-g
        pane size="25%" name="git" {
            command "lazygit"
        }
    }

    pane size=2 borderless=true {
        plugin location="zellij:status-bar"
    }
}
"#;

pub const RUST_LAYOUT_KDL: &str = r#"layout {
    pane size=1 borderless=true {
        plugin location="zellij:tab-bar"
    }

    pane split_direction="vertical" {
        pane size="18%" name="files" {
            command "yazi"
        }

        pane split_direction="horizontal" name="editor_col" {
            pane size="70%" name="editor" {
                command "helix"
                args "."
            }
            pane split_direction="vertical" size="30%" {
                pane name="shell" {
                    command "nu"
                }
                pane name="watch" {
                    command "nu"
                    args "-c" "cargo watch -x 'check --message-format short'"
                }
            }
        }

        pane size="25%" name="git" {
            command "lazygit"
        }
    }

    pane size=2 borderless=true {
        plugin location="zellij:status-bar"
    }
}
"#;

pub const NODE_LAYOUT_KDL: &str = r#"layout {
    pane size=1 borderless=true {
        plugin location="zellij:tab-bar"
    }

    pane split_direction="vertical" {
        pane size="18%" name="files" {
            command "yazi"
        }

        pane split_direction="horizontal" name="editor_col" {
            pane size="70%" name="editor" {
                command "helix"
                args "."
            }
            pane split_direction="vertical" size="30%" {
                pane name="shell" {
                    command "nu"
                }
                pane name="watch" {
                    command "nu"
                    args "-c" "npm run dev"
                }
            }
        }

        pane size="25%" name="git" {
            command "lazygit"
        }
    }

    pane size=2 borderless=true {
        plugin location="zellij:status-bar"
    }
}
"#;

pub const PYTHON_LAYOUT_KDL: &str = r#"layout {
    pane size=1 borderless=true {
        plugin location="zellij:tab-bar"
    }

    pane split_direction="vertical" {
        pane size="18%" name="files" {
            command "yazi"
        }

        pane split_direction="horizontal" name="editor_col" {
            pane size="70%" name="editor" {
                command "helix"
                args "."
            }
            pane split_direction="vertical" size="30%" {
                pane name="shell" {
                    command "nu"
                }
                pane name="watch" {
                    command "nu"
                    args "-c" "echo 'no watcher configured'"
                }
            }
        }

        pane size="25%" name="git" {
            command "lazygit"
        }
    }

    pane size=2 borderless=true {
        plugin location="zellij:status-bar"
    }
}
"#;

pub const NIX_LAYOUT_KDL: &str = r#"layout {
    pane size=1 borderless=true {
        plugin location="zellij:tab-bar"
    }

    pane split_direction="vertical" {
        pane size="18%" name="files" {
            command "yazi"
        }

        pane split_direction="horizontal" name="editor_col" {
            pane size="70%" name="editor" {
                command "helix"
                args "."
            }
            pane split_direction="vertical" size="30%" {
                pane name="shell" {
                    command "nu"
                }
                pane name="watch" {
                    command "nu"
                    args "-c" "echo 'no watcher configured'"
                }
            }
        }

        pane size="25%" name="nix" {
            command "nu"
            args "-c" "echo 'nix repl — use Ctrl-w to focus'"
        }
    }

    pane size=2 borderless=true {
        plugin location="zellij:status-bar"
    }
}
"#;

/// Write all bundled layouts to `~/.config/zllg/layouts/`.
pub fn write_bundled_layouts() -> Result<()> {
    let dir = layouts_dir();
    std::fs::create_dir_all(&dir)?;

    let layouts: &[(&str, &str)] = &[
        ("default", DEFAULT_LAYOUT_KDL),
        ("rust", RUST_LAYOUT_KDL),
        ("node", NODE_LAYOUT_KDL),
        ("python", PYTHON_LAYOUT_KDL),
        ("nix", NIX_LAYOUT_KDL),
    ];

    for (name, content) in layouts {
        let path = dir.join(format!("{name}.kdl"));
        if !path.exists() {
            std::fs::write(&path, content)?;
            println!("  wrote {}", path.display());
        } else {
            println!("  exists {}", path.display());
        }
    }

    Ok(())
}
