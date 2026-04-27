# zllg — Zellij IDE Orchestration Layer

> **zllg** = "zellige" without the name collision. A project-aware IDE session manager built on Zellij + WezTerm.

## What It Does

zllg detects your project type, generates a tailored KDL layout, and boots a multi-pane IDE session in Zellij with pane management, workspace routing, and a dashboard TUI.

```
zllg boot    →  detects project type →  loads KDL layout →  launches Zellij
```

## Quick Start

```bash
# Install (from source)
cargo install --path zllg

# First-time setup — scaffolds all config files
zllg init

# Verify toolchain
zllg check

# Boot an IDE session in the current directory
zllg boot

# Or specify a directory and layout
zllg boot -d /path/to/project -l rust
```

## Supported Project Types

| Marker File      | Type    | Layout Features                          |
|------------------|---------|------------------------------------------|
| `Cargo.toml`     | Rust    | `cargo watch` in watch pane              |
| `package.json`   | Node    | `npm run dev` in watch pane              |
| `pyproject.toml` | Python  | shell pane (no watcher by default)       |
| `flake.nix`      | Nix     | nix repl in right pane                   |
| *(none)*         | Default | generic shell panes                      |

## Architecture

```
zllg (CLI)
├── detect        → project type detection from directory markers
├── layout        → KDL layout generation (default, rust, node, python, nix)
├── config        → TOML config management (~/.config/zllg/)
├── pane          → Zellij pane operations (popout, toggle, move)
├── keybind       → Zellij keybind scaffolding
├── workspace     → WezTerm workspace config
├── dashboard     → IDE dashboard state builder
├── dashboard_tui → ratatui-based dashboard TUI
├── project_layout  → project-aware layout overrides
└── project_pane    → per-project pane command overrides
```

## Config Files

All config lives under `~/.config/zllg/`:

```
~/.config/zllg/
├── config.toml          # pane definitions, shell, workspaces
├── keybinds.toml        # Zellij keybind config
├── workspaces.toml      # WezTerm monitor mapping
├── project_layouts.toml # per-project-type layout overrides
└── layouts/             # generated KDL layout files
    ├── default.kdl
    ├── rust.kdl
    ├── node.kdl
    ├── python.kdl
    └── nix.kdl
```

## Default Layout

```
┌─────────────────────────────────────────────────────┐
│  Tab Bar                                            │
├────────┬──────────────────────────────┬─────────────┤
│        │                              │             │
│  files │      editor                  │     git     │
│ (yazi) │      (helix)                 │ (lazygit)   │
│        ├──────────────────────────────┤             │
│        │ shell    │ watch            │             │
│        │ (nu)     │ (cargo watch)    │             │
├────────┴──────────────────────────────┴─────────────┤
│  Status Bar                                         │
└─────────────────────────────────────────────────────┘
```

## Default Keybinds

| Key          | Action                        |
|--------------|-------------------------------|
| `Alt+f`      | Toggle files pane visibility  |
| `Alt+g`      | Toggle git pane visibility    |
| `Alt+e`      | Toggle editor pane visibility |
| `Alt+s`      | Toggle shell pane visibility  |
| `Ctrl+w`     | Focus pane with index         |
| `Alt+p`      | Toggle pane embed/eject       |

## CLI Reference

| Command             | Description                                    |
|---------------------|------------------------------------------------|
| `zllg boot`         | Start a Zellij IDE session                     |
| `zllg detect`       | Detect project type in a directory             |
| `zllg check`        | Verify required tools are installed            |
| `zllg init`         | Scaffold all config files                      |
| `zllg popout`       | Float a pane into a new WezTerm window         |
| `zllg duplicate`    | Duplicate a pane into a new window             |
| `zllg toggle`       | Toggle pane visibility                         |
| `zllg move-to`      | Move a pane to a WezTerm workspace             |
| `zllg dashboard`    | Print the IDE dashboard state                  |
| `zllg dashboard_tui`| Run the IDE dashboard TUI                      |
| `zllg keybinds`     | Scaffold Zellij keybinds                       |
| `zllg workspaces`   | Scaffold WezTerm workspace config              |
| `zllg list-workspaces` | List available workspaces                  |
| `zllg project-layouts` | Scaffold project-aware layouts             |
| `zllg config-path`  | Print the config file path                     |

## Requirements

**Required:**
- **zellij** — terminal multiplexer (IDE substrate)
- **wezterm** — terminal emulator (pop-out and workspace support)

**Optional (used in default panes):**
- **helix** — primary editor
- **yazi** — file manager
- **lazygit** — git TUI
- **nu** (Nushell) — shell pane and scripting
- **zsh** — POSIX fallback shell
- **cargo-watch** — Rust file watcher

## Development

```bash
# Build
cargo build -p zllg

# Test
cargo test -p zllg

# Lint
cargo clippy -p zllg -- -D warnings

# Format
cargo fmt -p zllg
```

## License

AGPL-3.0-or-later
