#![doc = include_str!("../README.md")]
//!
//! ## Modules
//!
//! - [`detection`] - Binary detection utilities
//! - [`error`] - Error types
//! - [`harness`] - Harness discovery and path resolution
//! - [`mcp`] - MCP server type definitions
//! - [`types`] - Core type definitions
//! - [`skill`] - Skill file parsing utilities
//! - [`validation`] - MCP server validation utilities

pub mod detection;
pub mod error;
pub mod harness;
pub mod mcp;
pub mod platform;
pub mod skill;
pub mod types;
pub mod validation;

pub use detection::find_binary;
pub use error::{Error, Result};
pub use harness::Harness;
pub use mcp::{
    HttpMcpServer, McpCapabilities, McpServer, OAuthConfig, SseMcpServer, StdioMcpServer,
};
pub use skill::{Frontmatter, Skill, parse_frontmatter, parse_skill};
pub use types::{
    ConfigResource, DirectoryResource, DirectoryStructure, EnvValue, FileFormat, HarnessKind,
    InstallationStatus, PathType, ResourceKind, Scope,
};
pub use validation::{
    AgentCapabilities, CODE_AGENT_COLOR_FORMAT, CODE_AGENT_MODE_UNSUPPORTED,
    CODE_AGENT_PARSE_ERROR, CODE_AGENT_TOOLS_FORMAT, CODE_AGENT_UNSUPPORTED,
    CODE_SKILL_DESCRIPTION_LENGTH, CODE_SKILL_DESCRIPTION_MISSING,
    CODE_SKILL_NAME_DIRECTORY_MISMATCH, CODE_SKILL_NAME_FORMAT, CODE_SKILL_NAME_LENGTH,
    CODE_SKILL_PARSE_ERROR, CODE_SKILL_UNSUPPORTED, ColorFormat, NameFormat,
    SKILL_DESCRIPTION_MAX_LEN, SKILL_NAME_MAX_LEN, SKILL_NAME_REGEX, Severity, SkillCapabilities,
    ToolsFormat, ValidationIssue, validate_agent_for_harness, validate_mcp_server,
    validate_skill_for_harness,
};
