# Changelog

All notable changes to `harness-locate` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.1] - 2026-01-16

### Added

- Support for GitHub Copilot CLI harness detection
- Updated crate description to include Copilot CLI

## [0.3.0] - 2025-01-04

### Added

- `Error::MissingEnvVar` variant for environment variable resolution failures
- `McpServer::to_native_value()` method for centralized harness-specific JSON serialization
- `McpServer::env_var_names()` method to extract referenced env var names from configs

### Changed

- Moved MCP-to-native conversion logic from `Harness::mcp_to_native_*` methods into `McpServer::to_native_value()`
- Fixed Goose OAuth capability test assertion (Goose doesn't support OAuth)

## [0.2.7] - 2024-12-31

### Added

- Initial workspace migration from standalone crate
- `HarnessLocator` trait for per-harness detection
- Support for Claude Code, OpenCode, Goose, and Amp Code harnesses
- Platform-specific path resolution (macOS, Linux, Windows)
- MCP server configuration parsing with Stdio, SSE, HTTP, and Docker variants
- Local skill file parsing from markdown

### Fixed

- SSE transport deprecation warnings for Claude Code

[Unreleased]: https://github.com/anthropics/harness-locate/compare/harness-locate-v0.4.1...HEAD
[0.4.1]: https://github.com/anthropics/harness-locate/compare/harness-locate-v0.3.0...harness-locate-v0.4.1
[0.3.0]: https://github.com/anthropics/harness-locate/compare/harness-locate-v0.2.7...harness-locate-v0.3.0
[0.2.7]: https://github.com/anthropics/harness-locate/releases/tag/harness-locate-v0.2.7
