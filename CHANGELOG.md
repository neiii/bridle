# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
