# skills-locate

Plugin discovery and fetching for AI coding harnesses (Claude Code, OpenCode, Goose, etc.).

Discovers and parses skills, commands, agents, hooks, and MCP servers from GitHub repositories.

## Installation

```toml
[dependencies]
skills-locate = "0.1"
```

## Quick Start

```rust
use skills_locate::{GitHubRef, discover_plugins};

// Parse a GitHub reference
let source = GitHubRef::parse("github:anthropics/claude-code")?;

// Discover all plugins from the repository
let plugins = discover_plugins(&source)?;

for plugin in plugins {
    println!("{}", plugin.name);
    
    for skill in &plugin.skills {
        println!("  skill: {}", skill.name);
    }
    for cmd in &plugin.commands {
        println!("  command: /{}", cmd.name);
    }
    for agent in &plugin.agents {
        println!("  agent: {}", agent.name);
    }
    for mcp in &plugin.mcp_servers {
        println!("  mcp: {}", mcp.name);
    }
}
```

## Components

| Type | Source | Description |
|------|--------|-------------|
| `SkillDescriptor` | `skills/*.md` | Reusable prompt templates |
| `CommandDescriptor` | `commands/*.md` | Slash commands |
| `AgentDescriptor` | `agents/*.md` | Subagent definitions |
| `HooksConfig` | `.claude-plugin/hooks.json` | Event hooks |
| `McpDescriptor` | `.claude-plugin/.mcp.json` | MCP server configs |

## Parsing Individual Files

```rust
use skills_locate::{parse_skill_descriptor, parse_command_descriptor, parse_agent_descriptor};

// Parse markdown files with YAML frontmatter
let skill = parse_skill_descriptor("review.md", content)?;
let command = parse_command_descriptor("deploy.md", content)?;
let agent = parse_agent_descriptor("researcher.md", content)?;
```

## GitHub References

```rust
use skills_locate::GitHubRef;

// Various formats supported
let r1 = GitHubRef::parse("github:owner/repo")?;
let r2 = GitHubRef::parse("github:owner/repo@v1.0.0")?;
let r3 = GitHubRef::parse("https://github.com/owner/repo")?;

// Access components
println!("{}/{} @ {}", r1.owner, r1.repo, r1.git_ref);
```

## Marketplace

```rust
use skills_locate::{Marketplace, fetch_json};

// Fetch a marketplace registry
let marketplace: Marketplace = fetch_json("https://example.com/marketplace.json")?;

for entry in &marketplace.plugins {
    println!("{}: {:?}", entry.name, entry.source);
}
```

## License

MIT
