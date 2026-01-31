# harness-locate

[![Crates.io](https://img.shields.io/crates/v/harness-locate.svg)](https://crates.io/crates/harness-locate)
[![Documentation](https://docs.rs/harness-locate/badge.svg)](https://docs.rs/harness-locate)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Cross-platform harness path discovery for AI coding CLI tools.

## Features

- Detect installed AI coding assistants (Claude Code, OpenCode, Goose, AMP Code, Copilot CLI, Crush)
- Resolve configuration paths (global and project-scoped)
- Unified MCP server configuration types
- Cross-platform support (macOS, Linux, Windows)

## Quick Start

### Detect Installed Harnesses

```rust,no_run
use harness_locate::{Harness, HarnessKind};

// Check all installed harnesses
for harness in Harness::installed()? {
    println!("{} is installed", harness.kind());
}
# Ok::<(), harness_locate::Error>(())
```

### Get Configuration Paths

```rust,no_run
use harness_locate::{Harness, HarnessKind, Scope};

let harness = Harness::locate(HarnessKind::ClaudeCode)?;
let config_dir = harness.config(&Scope::Global)?;
println!("Config at: {}", config_dir.display());
# Ok::<(), harness_locate::Error>(())
```

### MCP Server Configuration

```rust
use harness_locate::{Harness, HarnessKind};
use harness_locate::mcp::{McpServer, StdioMcpServer};

let server = McpServer::Stdio(StdioMcpServer {
    command: "npx".to_string(),
    args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()],
    env: Default::default(),
    cwd: None,
    enabled: true,
    timeout_ms: None,
});

// Check compatibility
let harness = Harness::new(HarnessKind::OpenCode);
if harness.supports_mcp_server(&server) {
    println!("Server is supported");
}
```

## Supported Harnesses

| Harness | Skills | Commands | MCP | Rules | Agents |
|---------|--------|----------|-----|-------|--------|
| Claude Code | Yes | Yes | Yes | Yes | Yes |
| OpenCode | Yes | Yes | Yes | Yes | Yes |
| Goose | Yes | No | Yes | Yes | No |
| AMP Code | Yes | Yes | Yes | Yes | No |
| Copilot CLI | Yes | No | Yes | Yes | Yes |
| Crush | Yes | No | Yes | Yes | No |

## Directory Naming Conventions

Different harnesses use different directory names. Use `HarnessKind::directory_names()` to query programmatically:

| Resource | OpenCode | Claude Code | Goose | AMP Code | Copilot CLI | Crush |
|----------|----------|-------------|-------|----------|-------------|-------|
| Skills   | `skill/` | `skills/`   | `skills/` | `skills/` | `skills/` | `skills/` |
| Commands | `command/`| `commands/` | -     | `commands/` | - | - |
| Agents   | `agent/` | `agents/`   | -     | -        | `agents/` | - |
| Plugins  | `plugin/`| `plugins/`  | -     | -        | - | - |

**Note:** Rules are stored at the root level, not in a named subdirectory.

**Note:** OpenCode uses singular names; all others use plural.

**Note:** Copilot CLI uses `.github/` for project-scoped agents and rules.

## Resource Types

### DirectoryResource

For directory-based resources (skills, commands):

- `path` - Directory location
- `exists` - Whether directory exists
- `structure` - Flat or Nested layout
- `file_format` - Expected file format

#### Directory Structure Patterns

**Skills** use **nested** structure (one subdirectory per skill):
```text
~/.config/opencode/skill/
  my-skill/
    SKILL.md
```

**Commands** use **flat** structure (files directly in directory):
```text
~/.config/opencode/command/
  my-command.md
  another-command.md
```

This pattern applies across all harnesses that support the resource type.

### ConfigResource

For file-based configuration (MCP):

- `file` - Config file path
- `key_path` - JSON pointer to relevant section
- `format` - JSON, YAML, etc.

## License

MIT
