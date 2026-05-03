# Reproducing the zllg workflow

## Source

The directory `/home/crombo/mirror-lab/zllg/` contains the zllg CLI (Zellij IDE orchestration layer — "zellige" without the name collision).

## Reproduction Steps

1. Build:
   ```sh
   cargo build -p zllg
   ```

2. Install (from source):
   ```sh
   cargo install --path zllg
   ```

3. First-time setup — scaffold all config files:
   ```sh
   zllg init
   ```

   Config files created under `~/.config/zllg/`:
   - `config.toml` — pane definitions, shell, workspaces
   - `keybinds.toml` — Zellij keybind config
   - `workspaces.toml` — WezTerm monitor mapping
   - `project_layouts.toml` — per-project-type layout overrides
   - `layouts/` — generated KDL layout files (default, rust, node, python, nix)

4. Verify toolchain:
   ```sh
   zllg check
   ```

5. Boot an IDE session in the current directory:
   ```sh
   zllg boot
   ```

6. Or specify a directory and layout:
   ```sh
   zllg boot -d /path/to/project -l rust
   ```

## CLI Commands

| Command | Description |
|---|---|
| `zllg boot` | Start a Zellij IDE session |
| `zllg detect` | Detect project type in a directory |
| `zllg check` | Verify required tools are installed |
| `zllg init` | Scaffold all config files |
| `zllg popout` | Float a pane into a new WezTerm window |
| `zllg duplicate` | Duplicate a pane into a new window |
| `zllg toggle` | Toggle pane visibility |
| `zllg move-to` | Move a pane to a WezTerm workspace |
| `zllg dashboard` | Print the IDE dashboard state |
| `zllg dashboard_tui` | Run the IDE dashboard TUI |
| `zllg keybinds` | Scaffold Zellij keybinds |
| `zllg workspaces` | Scaffold WezTerm workspace config |
| `zllg list-workspaces` | List available workspaces |
| `zllg project-layouts` | Scaffold project-aware layouts |
| `zllg config-path` | Print the config file path |

## Key Dependencies (Cargo.toml)

| Crate | Purpose |
|---|---|
| `clap` | CLI parsing |
| `anyhow` | top-level error propagation |
| `thiserror` | library error types |
| `serde`/`serde_json` | config serialization |
| `toml` | config file parsing |
| `dirs` | config path resolution |
| `which` | tool binary lookup |
| `crossterm` | terminal events |
| `ratatui` | dashboard TUI |
| `rusqlite` | SQLite persistence |
| `mirror-log` | event log integration |

## Dev Dependencies

| Crate | Purpose |
|---|---|
| `tempfile` | filesystem tests |

## Requirements

**Required:**
- `zellij` — terminal multiplexer (IDE substrate)
- `wezterm` — terminal emulator (pop-out and workspace support)

**Optional (used in default panes):**
- `helix` — primary editor
- `yazi` — file manager
- `lazygit` — git TUI
- `nu` (Nushell) — shell pane and scripting
- `zsh` — POSIX fallback shell
- `cargo-watch` — Rust file watcher

## Supported Project Types

| Marker File | Type | Layout Features |
|---|---|---|
| `Cargo.toml` | Rust | `cargo watch` in watch pane |
| `package.json` | Node | `npm run dev` in watch pane |
| `pyproject.toml` | Python | shell pane (no watcher by default) |
| `flake.nix` | Nix | nix repl in right pane |
| *(none)* | Default | generic shell panes |

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

| Key | Action |
|---|---|
| `Alt+f` | Toggle files pane visibility |
| `Alt+g` | Toggle git pane visibility |
| `Alt+e` | Toggle editor pane visibility |
| `Alt+s` | Toggle shell pane visibility |
| `Ctrl+w` | Focus pane with index |
| `Alt+p` | Toggle pane embed/eject |

## Manpages

Generated at build time via `clap_mangen::Man`. Output: `$OUT_DIR/zllg.1`.

## License

AGPL-3.0-or-later
