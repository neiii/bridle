mod agent;
mod command;
mod hook;
mod manifest;
mod mcp;
mod npm;
mod python;
mod skill;

pub use agent::{AgentDescriptor, parse_agent_descriptor};
pub use command::{CommandDescriptor, parse_command_descriptor};
#[allow(unused_imports)]
pub use hook::{HookAction, HookEvent, HookGroup, HooksConfig, parse_hooks_json};
pub use manifest::{ManifestConfig, parse_manifest};
pub use mcp::{McpServer, parse_mcp_json};
pub use npm::detect_npm_mcp;
pub use python::detect_python_mcp;
pub use skill::parse_skill_descriptor;
