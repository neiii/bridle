![Bridle](assets/bridle_header.png)

# Bridle

Unified configuration manager for AI coding assistants. Manage profiles, install skills/agents/commands, and switch configurations across Claude Code, OpenCode, Goose, and Amp.

> [!WARNING]
> If you're on a version before 0.2.2, please update immediately. Older versions had a critical bug that could cause data loss during profile switches.

## Installation

```bash
# Homebrew
brew install neiii/bridle/bridle

# Cargo
cargo install bridle

# From source
git clone https://github.com/neiii/bridle && cd bridle && cargo install --path .
```

## Quick Start

```bash
# Launch the TUI
bridle

# See what's configured across all harnesses
bridle status

# Create a profile from your current config
bridle profile create claude work --from-current

# Switch between profiles
bridle profile switch claude personal
```

![Screenshot](assets/screenshot.png)

## "Package Manager" for your harness

With Bridle, you're able to install skills, agents, commands, and MCPs from any GitHub repository, similar to how Claude Code does it. With Bridle, however, you're not limited to just one harness; we auto-translate all the paths, namings, schemas, and configurations for you. 

```bash
# Install from GitHub
bridle install owner/repo

# What happens:
# 1. Bridle scans the repo for skills, agents, commands, and MCPs
# 2. You select which components to install
# 3. You choose target harnesses and profiles
# 4. Bridle translates paths and configs for each harness automatically
```

**Why this matters:** A skill written for Claude Code uses `~/.claude/skills/`. The same skill on OpenCode lives at `~/.config/opencode/skill/`. MCPs follow different JSON/YAML schemas. Bridle handles all these differences for you.

| Component | Claude Code | OpenCode | Goose |
| --------- | ----------- | -------- | ----- |
| Skills    | `~/.claude/skills/` | `~/.config/opencode/skill/` | `~/.config/goose/skills/` |
| Agents    | `~/.claude/plugins/*/agents/` | `~/.config/opencode/agent/` | — |
| Commands  | `~/.claude/plugins/*/commands/` | `~/.config/opencode/command/` | — |
| MCPs      | `~/.claude/.mcp.json` | `opencode.jsonc` | `config.yaml` |

## Core Concepts

**Harnesses** are AI coding assistants: `claude`, `opencode`, `goose`, `amp`

**Profiles** are saved configurations. Each harness can have multiple profiles (e.g., `work`, `personal`, `minimal`). Bridle copies the active profile's config into the harness's config directory when you switch.

## Commands

### Status & TUI

| Command         | Description                                |
| --------------- | ------------------------------------------ |
| `bridle`        | Launch interactive TUI                     |
| `bridle status` | Show active profiles across all harnesses  |
| `bridle init`   | Initialize bridle config and default profiles |

### Profiles

| Command                                                 | Description                                 |
| ------------------------------------------------------- | ------------------------------------------- |
| `bridle profile list <harness>`                         | List all profiles for a harness             |
| `bridle profile show <harness> <name>`                  | Show profile details (model, MCPs, plugins) |
| `bridle profile create <harness> <name>`                | Create empty profile                        |
| `bridle profile create <harness> <name> --from-current` | Create profile from current config          |
| `bridle profile switch <harness> <name>`                | Activate a profile                          |
| `bridle profile edit <harness> <name>`                  | Open profile in editor                      |
| `bridle profile diff <harness> <name> [other]`          | Compare profiles                            |
| `bridle profile delete <harness> <name>`                | Delete a profile                            |

### Installing & Uninstalling

| Command                                | Description                                           |
| -------------------------------------- | ----------------------------------------------------- |
| `bridle install <source>`              | Install skills/MCPs from GitHub (`owner/repo` or URL) |
| `bridle install <source> --force`      | Overwrite existing installations                      |
| `bridle uninstall <harness> <profile>` | Interactively remove components [experimental]        |

### Configuration

| Command                           | Description          |
| --------------------------------- | -------------------- |
| `bridle config get <key>`         | Get a config value   |
| `bridle config set <key> <value>` | Set a config value   |

**Config keys:** `profile_marker`, `editor`, `tui.view`, `default_harness`

### Output Formats

All commands support `-o, --output <format>`:
- `text` (default) — Human-readable
- `json` — Machine-readable
- `auto` — Text for TTY, JSON for pipes

## Configuration

Bridle stores its config at `~/.config/bridle/config.toml`:

```toml
[active]
claude = "work"
opencode = "default"

profile_marker = false  # Create marker files for debugging
editor = "code --wait"  # Editor for `profile edit`

[tui]
view = "Dashboard"      # Will add more later :P 
```

## Supported Harnesses

| Harness     | Config Location         | Status       |
| ----------- | ----------------------- | ------------ |
| Claude Code | `~/.claude/`            | Full support |
| OpenCode    | `~/.config/opencode/`   | Full support |
| Goose       | `~/.config/goose/`      | Full support |
| Amp         | `~/.amp/`               | Experimental (ish) |

## Honorable Mentions
- Thank you Melvyn for [pointing out my stupidity](https://x.com/melvynxdev/status/2007312037920289275?s=20)
- Thank you Theo for [giving me a chance to show my work](https://x.com/theo/status/2007255666256425431)

## License

MIT
