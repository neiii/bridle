# Changelog

All notable changes to `skills-locate` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-01-16

### Changed

- Updated `harness-locate` dependency from 0.3.0 to 0.4.1 (adds Copilot CLI support)

## [0.2.0] - 2025-01-04

### Added

- `harness-locate` dependency for unified MCP type definitions
- `toml` dependency for pyproject.toml parsing
- `detect` module with `detect_mcp_from_files()` and confidence scoring (`High`/`Medium`/`Low`)
- `registry` module with `RegistryClient` for MCP registry API
- `manifest` module for MCPB desktop extension manifest parsing
- `npm` module with `detect_npm_mcp()` from package.json
- `python` module with `detect_python_mcp()` from pyproject.toml
- SSE/HTTP transport detection in `.mcp.json` parser
- `DetectedMcp`, `DetectionSource`, `DetectionConfidence` types

### Changed

- **BREAKING:** `parse_mcp_json()` returns `HashMap<String, McpServer>` instead of `Vec<McpDescriptor>`
- **BREAKING:** `PluginDescriptor.mcp_servers` field type changed from `Vec<McpDescriptor>` to `HashMap<String, McpServer>`
- **BREAKING:** `DiscoveryResult.all_mcp_servers` field type changed from `Vec` to `HashMap`
- Re-exports `McpServer` from `harness-locate` crate
- Collapsed nested if-let statements using Rust 2024 let-chains (clippy fix)

### Removed

- `McpDescriptor` type (replaced by unified `McpServer` from harness-locate)

## [0.1.1] - 2024-12-31

### Added

- Initial release as workspace crate
- GitHub URL parsing with `GitHubRef`
- Plugin discovery from GitHub repositories
- Marketplace JSON parsing
- HTTP fetching and ZIP archive extraction
- Skill, hook, command, and agent component parsing

[Unreleased]: https://github.com/anthropics/harness-locate/compare/skills-locate-v0.2.1...HEAD
[0.2.1]: https://github.com/anthropics/harness-locate/compare/skills-locate-v0.2.0...skills-locate-v0.2.1
[0.2.0]: https://github.com/anthropics/harness-locate/compare/skills-locate-v0.1.1...skills-locate-v0.2.0
[0.1.1]: https://github.com/anthropics/harness-locate/releases/tag/skills-locate-v0.1.1
