# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.7] - 2026-01-16

### Added

- **Copilot CLI harness support** - Full support for GitHub Copilot CLI as a new harness (#15)
  - Profile management (create, switch, show, delete)
  - MCP server installation to `~/.copilot/mcp-config.json`
  - Skills and agents installation
  - TUI integration
- **npm publishing** - Bridle is now available via npm for easier installation (#30)

### Changed

- Switch from git dependencies to versioned crates (`harness-locate 0.4.1`, `skills-locate 0.2.2`)
  - Enables `cargo install bridle` and crates.io publishing
  - Improves build reproducibility

## [0.2.6] - 2026-01-09

### Fixed

- Resolve TUI and profile switching performance issues (#25)
- Implement complete profile resource isolation (#24)

## [0.2.5] - 2026-01-06

### Added

- CI workflow with clippy, fmt, and test checks (#18, #19)

### Fixed

- Windows: editor spawning now uses `cmd /c` for `.cmd`/`.bat` wrappers like VS Code (#17)
- TUI edit no longer destroys unsaved changes in active profile
- Skills/agents/commands extraction uses harness-specific directory names (#20)

## [0.2.4] - 2026-01-04

### Added

- **MCP Installation System** - Full implementation of MCP server installation
  - MCP config read/write helpers for all harnesses
  - Core MCP installation function
  - CLI integration for MCP installation
  - YAML comment preservation for Goose config
- Update harness-locate to 0.3 and skills-locate to 0.2
- Integration tests for MCP installation
- Issue templates and contributing guide

### Fixed

- Editor commands with arguments now work correctly (e.g., `code --wait`)
- Terminal clears properly after GUI editor returns
- Remove local path patches for CI compatibility
- Bidirectional sync for configs

### Changed

- Claude Code MCP installation temporarily disabled (in development)

### Improved

- Show clean message while GUI editor is open

## [0.2.3] - 2026-01-03

### Changed

- **BREAKING**: Profile switching now provides full isolation
- Profiles are completely independent - switching profiles replaces ALL files in live config
- Current state is automatically saved to the old profile before switching (no data loss)

### Fixed

- Profile creation now sets the profile as active (enables proper save-before-switch)
- All files and directories (including hidden/dotfiles) are captured in profiles

### Migration

Users upgrading from 0.2.2 should be aware that runtime state (todos, transcripts, etc.) 
will now be profile-specific rather than shared across profiles.

## [0.2.2] - 2026-01-03

### Fixed

- **CRITICAL**: Profile switch no longer deletes unmanaged files (data loss bug affecting all harnesses)
- Profile creation/save now captures arbitrary directories, not just files
- Deep merge for managed directories preserves unknown nested files

### Security

- Fixed data loss vulnerability in profile switching (reported by @melvynxdev)

## [0.2.1] - 2026-01-03

### Fixed

- OpenCode skill installation now properly sanitizes skill names (e.g., "Hook Development" → "hook-development")

## [0.2.0] - 2026-01-02

### Added

- **Installation System**: Complete `install` command with interactive skill selection from GitHub repos
- Skill discovery module wrapping `skills-locate`
- Skill installation executor with path safety validation
- Agent and command discovery and installation
- MCP server discovery from GitHub repos
- Manifest tracking for installed components
- `uninstall` command for skills, agents, and commands
- `GroupMultiSelect` UI for profile selection
- Improved install UI and discovery for claude-code format
- Show disabled/warning states for incompatible agents
- Discord release notification workflow

### Fixed

- Use canonical dirs for profile storage, add harness writes for agents/commands
- Use canonical resource directory names in profile extraction
- Use harness-aware paths for profile resource sync
- Copy all subdirectories when creating profile from current
- TUI profile creation now copies all resources
- Check harness capability before installing agents/commands
- Transform skill names for OpenCode compatibility
- TUI: show skills/agents/commands for inactive profiles
- Replace path deps with published crates
- Update dialoguer imports to `dialoguer_multiselect`

### Documentation

- Add harness-locate agent validation spec

## [0.1.0] - 2025-12-31

### Added

- Initial public release
- Support for Claude Code, OpenCode, Goose, and AMP Code harnesses
- Profile management commands: list, show, create, delete, switch, edit, diff
- Terminal UI (TUI) dashboard with keyboard and mouse support
- CLI with JSON output support for scripting
- MCP server configuration parsing and display
- Plugin/extension configuration parsing
- Commands and skills extraction
