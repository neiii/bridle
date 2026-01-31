//! MCP server configuration validation.
//!
//! This module provides validation for [`McpServer`] configurations,
//! checking for structural issues like empty commands, invalid URLs,
//! excessive timeouts, and suspicious environment variable names.
//!
//! Unlike the fail-fast error handling elsewhere in this crate,
//! validation collects all issues found, allowing callers to see
//! the complete picture rather than stopping at the first problem.
//!
//! # Example
//!
//! ```
//! use harness_locate::mcp::{McpServer, StdioMcpServer};
//! use harness_locate::validation::{validate_mcp_server, Severity};
//!
//! let server = McpServer::Stdio(StdioMcpServer {
//!     command: String::new(), // Empty command - will be flagged
//!     args: vec![],
//!     env: std::collections::HashMap::new(),
//!     cwd: None,
//!     enabled: true,
//!     timeout_ms: None,
//! });
//!
//! let issues = validate_mcp_server(&server);
//! assert!(!issues.is_empty());
//! assert!(issues.iter().any(|i| i.severity == Severity::Error));
//! ```

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::mcp::{HttpMcpServer, McpCapabilities, McpServer, SseMcpServer, StdioMcpServer};
use crate::types::{EnvValue, HarnessKind};

static SKILL_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(SKILL_NAME_REGEX).expect("invalid skill name regex"));

// Issue code constants for machine-readable classification.

/// Empty command in stdio transport.
pub const CODE_EMPTY_COMMAND: &str = "stdio.command.empty";

/// URL failed to parse.
pub const CODE_INVALID_URL: &str = "url.invalid";

/// URL has non-http(s) scheme.
pub const CODE_INVALID_SCHEME: &str = "url.scheme.invalid";

/// Timeout exceeds recommended maximum.
pub const CODE_TIMEOUT_EXCESSIVE: &str = "timeout.excessive";

/// Environment variable name suggests sensitive data.
pub const CODE_SUSPICIOUS_ENV: &str = "env.suspicious_name";

/// Working directory (cwd) not supported by harness.
pub const CODE_CWD_UNSUPPORTED: &str = "harness.cwd.unsupported";

/// Toggle (enabled field) not supported by harness.
pub const CODE_TOGGLE_UNSUPPORTED: &str = "harness.toggle.unsupported";

/// SSE transport deprecated for this harness (prefer HTTP).
pub const CODE_SSE_DEPRECATED: &str = "harness.transport.sse_deprecated";

// Agent validation codes.

/// Agent tools field has wrong type for harness.
pub const CODE_AGENT_TOOLS_FORMAT: &str = "agent.tools.format";

/// Agent color field has invalid format for harness.
pub const CODE_AGENT_COLOR_FORMAT: &str = "agent.color.format";

/// Agent mode value not supported by harness.
pub const CODE_AGENT_MODE_UNSUPPORTED: &str = "agent.mode.unsupported";

/// Harness does not support agents.
pub const CODE_AGENT_UNSUPPORTED: &str = "agent.unsupported";

/// Agent frontmatter failed to parse.
pub const CODE_AGENT_PARSE_ERROR: &str = "agent.parse_error";

// Skill validation codes.

/// Skill name has invalid format for harness.
pub const CODE_SKILL_NAME_FORMAT: &str = "skill.name.invalid_format";

/// Skill name exceeds maximum length.
pub const CODE_SKILL_NAME_LENGTH: &str = "skill.name.length";

/// Skill description exceeds maximum length.
pub const CODE_SKILL_DESCRIPTION_LENGTH: &str = "skill.description.length";

/// Skill name does not match directory name.
pub const CODE_SKILL_NAME_DIRECTORY_MISMATCH: &str = "skill.name.directory_mismatch";

/// Harness does not support skills.
pub const CODE_SKILL_UNSUPPORTED: &str = "skill.unsupported";

/// Skill frontmatter failed to parse.
pub const CODE_SKILL_PARSE_ERROR: &str = "skill.parse_error";

/// Skill is missing required description field.
pub const CODE_SKILL_DESCRIPTION_MISSING: &str = "skill.description.missing";

/// Skill name validation regex: lowercase alphanumeric with single hyphens.
pub const SKILL_NAME_REGEX: &str = r"^[a-z0-9]+(-[a-z0-9]+)*$";

/// Maximum length for skill name.
pub const SKILL_NAME_MAX_LEN: usize = 64;

/// Maximum length for skill description.
pub const SKILL_DESCRIPTION_MAX_LEN: usize = 1024;

/// Severity level for validation issues.
///
/// Determines how the issue should be treated by callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// Critical issue that will likely cause the server to fail.
    ///
    /// Examples: empty command, unparseable URL.
    Error,

    /// Non-critical issue that may cause problems or is worth reviewing.
    ///
    /// Examples: very long timeout, suspicious environment variable name.
    Warning,
}

/// Expected format for agent `tools` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolsFormat {
    /// `Record<string, boolean>` - OpenCode style: `{ bash: true, edit: false }`
    BooleanRecord,
    /// Comma-separated string - Claude Code style: `"Glob, Grep, Read"`
    CommaSeparatedString,
}

/// Expected format for agent `color` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorFormat {
    /// Only hex colors: `#RRGGBB`
    HexOnly,
    /// Named colors (red, blue) or hex - accepts any string.
    NamedOrHex,
}

/// Describes agent validation requirements for a harness.
#[derive(Debug, Clone)]
pub struct AgentCapabilities {
    /// Expected format for `tools` field.
    pub tools_format: ToolsFormat,
    /// Expected format for `color` field.
    pub color_format: ColorFormat,
    /// Supported mode values.
    pub supported_modes: &'static [&'static str],
}

impl AgentCapabilities {
    #[must_use]
    pub fn for_kind(kind: HarnessKind) -> Option<Self> {
        match kind {
            HarnessKind::OpenCode => Some(Self {
                tools_format: ToolsFormat::BooleanRecord,
                color_format: ColorFormat::HexOnly,
                supported_modes: &["subagent", "primary", "all"],
            }),
            HarnessKind::ClaudeCode | HarnessKind::AmpCode => Some(Self {
                tools_format: ToolsFormat::CommaSeparatedString,
                color_format: ColorFormat::NamedOrHex,
                supported_modes: &["subagent", "primary"],
            }),
            HarnessKind::CopilotCli | HarnessKind::Droid => Some(Self {
                tools_format: ToolsFormat::CommaSeparatedString,
                color_format: ColorFormat::NamedOrHex,
                supported_modes: &["subagent", "primary"],
            }),
            HarnessKind::Goose | HarnessKind::Crush => None,
        }
    }
}

/// Expected format for skill `name` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameFormat {
    /// Lowercase alphanumeric with hyphens only: `^[a-z0-9]+(-[a-z0-9]+)*$`
    LowercaseHyphenated,
    /// Any string format accepted.
    Any,
}

/// Describes skill validation requirements for a harness.
#[derive(Debug, Clone)]
pub struct SkillCapabilities {
    /// Expected format for `name` field.
    pub name_format: NameFormat,
    /// Whether skill name must match parent directory name.
    pub name_must_match_directory: bool,
    /// Whether description field is required.
    pub description_required: bool,
}

impl SkillCapabilities {
    #[must_use]
    pub fn for_kind(kind: HarnessKind) -> Option<Self> {
        match kind {
            HarnessKind::OpenCode => Some(Self {
                name_format: NameFormat::LowercaseHyphenated,
                name_must_match_directory: true,
                description_required: true,
            }),
            HarnessKind::ClaudeCode | HarnessKind::AmpCode | HarnessKind::Droid => Some(Self {
                name_format: NameFormat::Any,
                name_must_match_directory: false,
                description_required: false,
            }),
            // Copilot CLI follows agentskills.io spec: lowercase hyphenated names,
            // name must match directory, description required
            HarnessKind::CopilotCli => Some(Self {
                name_format: NameFormat::LowercaseHyphenated,
                name_must_match_directory: true,
                description_required: true,
            }),
            HarnessKind::Goose => None,
            HarnessKind::Crush => Some(Self {
                name_format: NameFormat::Any,
                name_must_match_directory: false,
                description_required: false,
            }),
        }
    }
}

/// A validation issue found in an MCP server configuration.
///
/// Issues are collected by [`validate_mcp_server`] and returned as a `Vec`.
/// An empty result means the configuration passed all checks.
///
/// # Extensibility
///
/// This struct is marked `#[non_exhaustive]` to allow adding new fields
/// in future versions without breaking changes. Use the constructor
/// methods [`ValidationIssue::error`] and [`ValidationIssue::warning`]
/// rather than constructing directly.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// Severity of the issue.
    pub severity: Severity,

    /// The field path where the issue was found (e.g., "command", "url", "env.SECRET_KEY").
    pub field: String,

    /// Human-readable description of the issue.
    pub message: String,

    /// Machine-readable issue code for programmatic filtering.
    ///
    /// See the `CODE_*` constants in this module.
    pub code: Option<&'static str>,
}

impl ValidationIssue {
    /// Creates an error-level validation issue.
    ///
    /// # Arguments
    ///
    /// * `field` - The field path where the issue was found
    /// * `message` - Human-readable description
    /// * `code` - Optional machine-readable code
    #[must_use]
    pub fn error(
        field: impl Into<String>,
        message: impl Into<String>,
        code: Option<&'static str>,
    ) -> Self {
        Self {
            severity: Severity::Error,
            field: field.into(),
            message: message.into(),
            code,
        }
    }

    /// Creates a warning-level validation issue.
    ///
    /// # Arguments
    ///
    /// * `field` - The field path where the issue was found
    /// * `message` - Human-readable description
    /// * `code` - Optional machine-readable code
    #[must_use]
    pub fn warning(
        field: impl Into<String>,
        message: impl Into<String>,
        code: Option<&'static str>,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            field: field.into(),
            message: message.into(),
            code,
        }
    }
}

/// Maximum recommended timeout in milliseconds (5 minutes).
const MAX_RECOMMENDED_TIMEOUT_MS: u64 = 300_000;

/// Patterns that suggest an environment variable contains sensitive data.
///
/// These are checked case-insensitively against variable names.
const SUSPICIOUS_ENV_PATTERNS: &[&str] = &[
    "PASSWORD",
    "PASSWD",
    "SECRET",
    "TOKEN",
    "API_KEY",
    "PRIVATE_KEY",
    "ACCESS_KEY",
    "CREDENTIAL",
    "BEARER",
    "AUTH",
];

/// Validates an MCP server configuration.
///
/// Checks for structural issues like empty commands, invalid URLs,
/// excessive timeouts, and suspicious environment variable names.
/// Returns all issues found, allowing callers to see the complete picture.
///
/// # Arguments
///
/// * `server` - The MCP server configuration to validate
///
/// # Returns
///
/// A vector of validation issues. An empty vector means no issues were found.
///
/// # Example
///
/// ```
/// use harness_locate::mcp::{McpServer, StdioMcpServer};
/// use harness_locate::validation::validate_mcp_server;
///
/// let server = McpServer::Stdio(StdioMcpServer {
///     command: "node".to_string(),
///     args: vec!["server.js".to_string()],
///     env: std::collections::HashMap::new(),
///     cwd: None,
///     enabled: true,
///     timeout_ms: None,
/// });
///
/// let issues = validate_mcp_server(&server);
/// assert!(issues.is_empty()); // Valid configuration
/// ```
#[must_use]
pub fn validate_mcp_server(server: &McpServer) -> Vec<ValidationIssue> {
    match server {
        McpServer::Stdio(s) => validate_stdio(s),
        McpServer::Sse(s) => validate_sse(s),
        McpServer::Http(s) => validate_http(s),
    }
}

/// Validates an MCP server configuration for a specific harness.
///
/// Combines base validation with harness-specific capability checks.
/// Returns all issues found, including structural problems and harness incompatibilities.
#[must_use]
pub fn validate_for_harness(server: &McpServer, kind: HarnessKind) -> Vec<ValidationIssue> {
    let mut issues = validate_mcp_server(server);
    let caps = McpCapabilities::for_kind(kind);
    let harness_name = kind.as_str();

    match server {
        McpServer::Stdio(s) => {
            if s.cwd.is_some() && !caps.cwd {
                issues.push(ValidationIssue::error(
                    "cwd",
                    format!("Working directory not supported by {harness_name}"),
                    Some(CODE_CWD_UNSUPPORTED),
                ));
            }
            if !s.enabled && !caps.toggle {
                issues.push(ValidationIssue::warning(
                    "enabled",
                    format!("{harness_name} ignores the enabled field; server will always run"),
                    Some(CODE_TOGGLE_UNSUPPORTED),
                ));
            }
        }
        McpServer::Sse(s) => {
            if kind == HarnessKind::ClaudeCode {
                issues.push(ValidationIssue::warning(
                    "transport",
                    "SSE transport works but HTTP is preferred for Claude Code",
                    Some(CODE_SSE_DEPRECATED),
                ));
            }
            if !s.enabled && !caps.toggle {
                issues.push(ValidationIssue::warning(
                    "enabled",
                    format!("{harness_name} ignores the enabled field; server will always run"),
                    Some(CODE_TOGGLE_UNSUPPORTED),
                ));
            }
        }
        McpServer::Http(s) => {
            if !s.enabled && !caps.toggle {
                issues.push(ValidationIssue::warning(
                    "enabled",
                    format!("{harness_name} ignores the enabled field; server will always run"),
                    Some(CODE_TOGGLE_UNSUPPORTED),
                ));
            }
        }
    }

    issues
}

/// Validates agent frontmatter content for a specific harness.
///
/// Returns an empty vector if valid, or a list of issues found.
/// Returns a single `CODE_AGENT_UNSUPPORTED` error if harness doesn't support agents.
#[must_use]
pub fn validate_agent_for_harness(content: &str, kind: HarnessKind) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    let caps = match AgentCapabilities::for_kind(kind) {
        Some(c) => c,
        None => {
            issues.push(ValidationIssue::error(
                "agent",
                format!("{} does not support agents", kind.as_str()),
                Some(CODE_AGENT_UNSUPPORTED),
            ));
            return issues;
        }
    };

    let frontmatter = match crate::skill::parse_frontmatter(content) {
        Ok(fm) => fm,
        Err(e) => {
            issues.push(ValidationIssue::error(
                "frontmatter",
                format!("failed to parse frontmatter: {e}"),
                Some(CODE_AGENT_PARSE_ERROR),
            ));
            return issues;
        }
    };

    let yaml = match &frontmatter.yaml {
        Some(y) => y,
        None => return issues,
    };

    if let Some(tools) = yaml.get("tools") {
        issues.extend(validate_tools_format(tools, caps.tools_format, kind));
    }

    if let Some(color) = yaml.get("color").and_then(|v| v.as_str()) {
        issues.extend(validate_color_format(color, caps.color_format, kind));
    }

    if let Some(mode) = yaml.get("mode").and_then(|v| v.as_str())
        && !caps.supported_modes.contains(&mode)
    {
        issues.push(ValidationIssue::error(
            "mode",
            format!(
                "mode '{}' not supported by {}; valid: {:?}",
                mode,
                kind.as_str(),
                caps.supported_modes
            ),
            Some(CODE_AGENT_MODE_UNSUPPORTED),
        ));
    }

    issues
}

/// Validates skill frontmatter content for a specific harness.
///
/// Returns an empty vector if valid, or a list of issues found.
/// Returns a single `CODE_SKILL_UNSUPPORTED` error if harness doesn't support skills.
#[must_use]
pub fn validate_skill_for_harness(
    content: &str,
    directory_name: &str,
    kind: HarnessKind,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    let caps = match SkillCapabilities::for_kind(kind) {
        Some(c) => c,
        None => {
            issues.push(ValidationIssue::error(
                "skill",
                format!("{} does not support skills", kind.as_str()),
                Some(CODE_SKILL_UNSUPPORTED),
            ));
            return issues;
        }
    };

    let frontmatter = match crate::skill::parse_frontmatter(content) {
        Ok(fm) => fm,
        Err(e) => {
            issues.push(ValidationIssue::error(
                "frontmatter",
                format!("failed to parse frontmatter: {e}"),
                Some(CODE_SKILL_PARSE_ERROR),
            ));
            return issues;
        }
    };

    let yaml = match &frontmatter.yaml {
        Some(y) => y,
        None => return issues,
    };

    if let Some(name) = yaml.get("name").and_then(|v| v.as_str()) {
        if caps.name_format == NameFormat::LowercaseHyphenated && !SKILL_NAME_RE.is_match(name) {
            issues.push(ValidationIssue::error(
                "name",
                format!(
                    "name '{}' must be lowercase alphanumeric with hyphens (regex: {})",
                    name, SKILL_NAME_REGEX
                ),
                Some(CODE_SKILL_NAME_FORMAT),
            ));
        }

        if name.len() > SKILL_NAME_MAX_LEN {
            issues.push(ValidationIssue::error(
                "name",
                format!("name exceeds {} characters", SKILL_NAME_MAX_LEN),
                Some(CODE_SKILL_NAME_LENGTH),
            ));
        }

        if caps.name_must_match_directory && name != directory_name {
            issues.push(ValidationIssue::error(
                "name",
                format!(
                    "name '{}' must match directory name '{}'",
                    name, directory_name
                ),
                Some(CODE_SKILL_NAME_DIRECTORY_MISMATCH),
            ));
        }
    }

    if let Some(description) = yaml.get("description").and_then(|v| v.as_str()) {
        if description.len() > SKILL_DESCRIPTION_MAX_LEN {
            issues.push(ValidationIssue::error(
                "description",
                format!(
                    "description exceeds {} characters",
                    SKILL_DESCRIPTION_MAX_LEN
                ),
                Some(CODE_SKILL_DESCRIPTION_LENGTH),
            ));
        }
    } else if caps.description_required {
        issues.push(ValidationIssue::warning(
            "description",
            format!("{} recommends a description field", kind.as_str()),
            Some(CODE_SKILL_DESCRIPTION_MISSING),
        ));
    }

    issues
}

fn validate_tools_format(
    tools: &serde_yaml::Value,
    expected: ToolsFormat,
    kind: HarnessKind,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    match expected {
        ToolsFormat::BooleanRecord => {
            if !tools.is_mapping() {
                issues.push(ValidationIssue::error(
                    "tools",
                    format!(
                        "{} requires tools as object (e.g., {{ bash: true }}), got {}",
                        kind.as_str(),
                        yaml_type_name(tools)
                    ),
                    Some(CODE_AGENT_TOOLS_FORMAT),
                ));
            }
        }
        ToolsFormat::CommaSeparatedString => {
            if !tools.is_string() {
                issues.push(ValidationIssue::error(
                    "tools",
                    format!(
                        "{} requires tools as comma-separated string, got {}",
                        kind.as_str(),
                        yaml_type_name(tools)
                    ),
                    Some(CODE_AGENT_TOOLS_FORMAT),
                ));
            }
        }
    }

    issues
}

fn validate_color_format(
    color: &str,
    expected: ColorFormat,
    kind: HarnessKind,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    match expected {
        ColorFormat::HexOnly => {
            if !is_hex_color(color) {
                issues.push(ValidationIssue::error(
                    "color",
                    format!(
                        "{} requires hex color (#RRGGBB), got '{}'",
                        kind.as_str(),
                        color
                    ),
                    Some(CODE_AGENT_COLOR_FORMAT),
                ));
            }
        }
        ColorFormat::NamedOrHex => {}
    }

    issues
}

fn yaml_type_name(value: &serde_yaml::Value) -> &'static str {
    match value {
        serde_yaml::Value::Null => "null",
        serde_yaml::Value::Bool(_) => "boolean",
        serde_yaml::Value::Number(_) => "number",
        serde_yaml::Value::String(_) => "string",
        serde_yaml::Value::Sequence(_) => "array",
        serde_yaml::Value::Mapping(_) => "object",
        serde_yaml::Value::Tagged(_) => "tagged",
    }
}

fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn validate_stdio(server: &StdioMcpServer) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if server.command.trim().is_empty() {
        issues.push(ValidationIssue::error(
            "command",
            "Command must not be empty",
            Some(CODE_EMPTY_COMMAND),
        ));
    }

    issues.extend(validate_timeout(server.timeout_ms, "timeout_ms"));
    issues.extend(validate_env(&server.env, "env"));
    issues
}

fn validate_sse(server: &SseMcpServer) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    issues.extend(validate_url(&server.url, "url"));
    issues.extend(validate_timeout(server.timeout_ms, "timeout_ms"));
    issues.extend(validate_env(&server.headers, "headers"));
    issues
}

fn validate_http(server: &HttpMcpServer) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    issues.extend(validate_url(&server.url, "url"));
    issues.extend(validate_timeout(server.timeout_ms, "timeout_ms"));
    issues.extend(validate_env(&server.headers, "headers"));
    issues
}
fn validate_url(url: &str, field: &str) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    match Url::parse(url) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            if scheme != "http" && scheme != "https" {
                issues.push(ValidationIssue::error(
                    field,
                    format!("URL scheme must be http or https, got '{scheme}'"),
                    Some(CODE_INVALID_SCHEME),
                ));
            }
        }
        Err(e) => {
            issues.push(ValidationIssue::error(
                field,
                format!("Invalid URL: {e}"),
                Some(CODE_INVALID_URL),
            ));
        }
    }

    issues
}

fn validate_timeout(timeout_ms: Option<u64>, field: &str) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if let Some(ms) = timeout_ms
        && ms > MAX_RECOMMENDED_TIMEOUT_MS
    {
        issues.push(ValidationIssue::warning(
            field,
            format!(
                "Timeout of {}ms exceeds recommended maximum of {}ms (5 minutes)",
                ms, MAX_RECOMMENDED_TIMEOUT_MS
            ),
            Some(CODE_TIMEOUT_EXCESSIVE),
        ));
    }

    issues
}

fn validate_env(env: &HashMap<String, EnvValue>, field_prefix: &str) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    for key in env.keys() {
        let upper = key.to_uppercase();
        for pattern in SUSPICIOUS_ENV_PATTERNS {
            if upper.contains(pattern) {
                issues.push(ValidationIssue::warning(
                    format!("{field_prefix}.{key}"),
                    format!(
                        "Variable name '{key}' suggests sensitive data; \
                         consider using environment variable references"
                    ),
                    Some(CODE_SUSPICIOUS_ENV),
                ));
                break;
            }
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stdio(command: &str) -> McpServer {
        McpServer::Stdio(StdioMcpServer {
            command: command.to_string(),
            args: vec![],
            env: HashMap::new(),
            cwd: None,
            enabled: true,
            timeout_ms: None,
        })
    }

    fn make_sse(url: &str) -> McpServer {
        McpServer::Sse(SseMcpServer {
            url: url.to_string(),
            headers: HashMap::new(),
            enabled: true,
            timeout_ms: None,
        })
    }

    fn make_http(url: &str) -> McpServer {
        McpServer::Http(HttpMcpServer {
            url: url.to_string(),
            headers: HashMap::new(),
            oauth: None,
            enabled: true,
            timeout_ms: None,
        })
    }

    #[test]
    fn empty_command_returns_error() {
        let server = make_stdio("");
        let issues = validate_mcp_server(&server);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
        assert_eq!(issues[0].field, "command");
        assert_eq!(issues[0].code, Some(CODE_EMPTY_COMMAND));
    }

    #[test]
    fn valid_command_returns_no_issues() {
        let server = make_stdio("node");
        let issues = validate_mcp_server(&server);

        assert!(issues.is_empty());
    }

    #[test]
    fn invalid_url_returns_error() {
        let server = make_sse("not-a-valid-url");
        let issues = validate_mcp_server(&server);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
        assert_eq!(issues[0].field, "url");
        assert_eq!(issues[0].code, Some(CODE_INVALID_URL));
    }

    #[test]
    fn valid_https_url_returns_no_issues() {
        let server = make_http("https://example.com/mcp");
        let issues = validate_mcp_server(&server);

        assert!(issues.is_empty());
    }

    #[test]
    fn ftp_scheme_returns_error() {
        let server = make_sse("ftp://files.example.com");
        let issues = validate_mcp_server(&server);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
        assert_eq!(issues[0].field, "url");
        assert_eq!(issues[0].code, Some(CODE_INVALID_SCHEME));
        assert!(issues[0].message.contains("ftp"));
    }

    #[test]
    fn excessive_timeout_returns_warning() {
        let server = McpServer::Stdio(StdioMcpServer {
            command: "node".to_string(),
            args: vec![],
            env: HashMap::new(),
            cwd: None,
            enabled: true,
            timeout_ms: Some(600_000),
        });
        let issues = validate_mcp_server(&server);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(issues[0].field, "timeout_ms");
        assert_eq!(issues[0].code, Some(CODE_TIMEOUT_EXCESSIVE));
    }

    #[test]
    fn normal_timeout_returns_no_issues() {
        let server = McpServer::Stdio(StdioMcpServer {
            command: "node".to_string(),
            args: vec![],
            env: HashMap::new(),
            cwd: None,
            enabled: true,
            timeout_ms: Some(30_000),
        });
        let issues = validate_mcp_server(&server);

        assert!(issues.is_empty());
    }

    #[test]
    fn suspicious_env_name_returns_warning() {
        let mut env = HashMap::new();
        env.insert("DB_PASSWORD".to_string(), EnvValue::plain("secret123"));

        let server = McpServer::Stdio(StdioMcpServer {
            command: "node".to_string(),
            args: vec![],
            env,
            cwd: None,
            enabled: true,
            timeout_ms: None,
        });
        let issues = validate_mcp_server(&server);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(issues[0].field, "env.DB_PASSWORD");
        assert_eq!(issues[0].code, Some(CODE_SUSPICIOUS_ENV));
    }

    #[test]
    fn normal_env_name_returns_no_issues() {
        let mut env = HashMap::new();
        env.insert("NODE_ENV".to_string(), EnvValue::plain("production"));
        env.insert("PORT".to_string(), EnvValue::plain("3000"));

        let server = McpServer::Stdio(StdioMcpServer {
            command: "node".to_string(),
            args: vec![],
            env,
            cwd: None,
            enabled: true,
            timeout_ms: None,
        });
        let issues = validate_mcp_server(&server);

        assert!(issues.is_empty());
    }

    #[test]
    fn multiple_issues_collected() {
        let mut env = HashMap::new();
        env.insert("API_TOKEN".to_string(), EnvValue::plain("tok_123"));

        let server = McpServer::Stdio(StdioMcpServer {
            command: "".to_string(),
            args: vec![],
            env,
            cwd: None,
            enabled: true,
            timeout_ms: Some(600_000),
        });
        let issues = validate_mcp_server(&server);

        assert_eq!(issues.len(), 3);
        let error_count = issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count();
        let warning_count = issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count();
        assert_eq!(error_count, 1);
        assert_eq!(warning_count, 2);
        assert!(issues.iter().any(|i| i.code == Some(CODE_EMPTY_COMMAND)));
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_TIMEOUT_EXCESSIVE))
        );
        assert!(issues.iter().any(|i| i.code == Some(CODE_SUSPICIOUS_ENV)));
    }

    #[test]
    fn valid_config_returns_empty_vec() {
        let mut env = HashMap::new();
        env.insert("NODE_ENV".to_string(), EnvValue::plain("production"));

        let server = McpServer::Stdio(StdioMcpServer {
            command: "node".to_string(),
            args: vec!["server.js".to_string()],
            env,
            cwd: None,
            enabled: true,
            timeout_ms: Some(30_000),
        });
        let issues = validate_mcp_server(&server);

        assert!(issues.is_empty());
    }

    // Harness-specific validation tests

    #[test]
    fn cwd_on_any_harness_returns_error() {
        let server = McpServer::Stdio(StdioMcpServer {
            command: "node".to_string(),
            args: vec![],
            env: HashMap::new(),
            cwd: Some(std::path::PathBuf::from("/tmp")),
            enabled: true,
            timeout_ms: None,
        });

        for kind in HarnessKind::ALL {
            let issues = validate_for_harness(&server, *kind);
            assert!(issues.iter().any(|i| i.code == Some(CODE_CWD_UNSUPPORTED)));
        }
    }

    #[test]
    fn disabled_on_claude_code_returns_warning() {
        let server = McpServer::Stdio(StdioMcpServer {
            command: "node".to_string(),
            args: vec![],
            env: HashMap::new(),
            cwd: None,
            enabled: false,
            timeout_ms: None,
        });

        let issues = validate_for_harness(&server, HarnessKind::ClaudeCode);
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_TOGGLE_UNSUPPORTED))
        );
    }

    #[test]
    fn disabled_on_opencode_returns_no_warning() {
        let server = McpServer::Stdio(StdioMcpServer {
            command: "node".to_string(),
            args: vec![],
            env: HashMap::new(),
            cwd: None,
            enabled: false,
            timeout_ms: None,
        });

        let issues = validate_for_harness(&server, HarnessKind::OpenCode);
        assert!(
            !issues
                .iter()
                .any(|i| i.code == Some(CODE_TOGGLE_UNSUPPORTED))
        );
    }

    #[test]
    fn sse_on_claude_code_returns_warning() {
        let server = McpServer::Sse(SseMcpServer {
            url: "https://example.com/sse".to_string(),
            headers: HashMap::new(),
            enabled: true,
            timeout_ms: None,
        });

        let issues = validate_for_harness(&server, HarnessKind::ClaudeCode);
        assert!(issues.iter().any(|i| i.code == Some(CODE_SSE_DEPRECATED)));
    }

    #[test]
    fn sse_on_opencode_returns_no_warning() {
        let server = McpServer::Sse(SseMcpServer {
            url: "https://example.com/sse".to_string(),
            headers: HashMap::new(),
            enabled: true,
            timeout_ms: None,
        });

        let issues = validate_for_harness(&server, HarnessKind::OpenCode);
        assert!(!issues.iter().any(|i| i.code == Some(CODE_SSE_DEPRECATED)));
    }

    #[test]
    fn validate_for_harness_includes_base_validation() {
        let server = McpServer::Stdio(StdioMcpServer {
            command: "".to_string(),
            args: vec![],
            env: HashMap::new(),
            cwd: Some(std::path::PathBuf::from("/tmp")),
            enabled: true,
            timeout_ms: None,
        });

        let issues = validate_for_harness(&server, HarnessKind::ClaudeCode);
        assert!(issues.iter().any(|i| i.code == Some(CODE_EMPTY_COMMAND)));
        assert!(issues.iter().any(|i| i.code == Some(CODE_CWD_UNSUPPORTED)));
    }

    // Agent validation tests

    #[test]
    fn opencode_rejects_comma_string_tools() {
        let content = "---\ntools: Glob, Grep, Read\n---\nAgent prompt";
        let issues = validate_agent_for_harness(content, HarnessKind::OpenCode);
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_AGENT_TOOLS_FORMAT))
        );
    }

    #[test]
    fn opencode_accepts_boolean_record_tools() {
        let content = "---\ntools:\n  bash: true\n  edit: false\n---\nAgent prompt";
        let issues = validate_agent_for_harness(content, HarnessKind::OpenCode);
        assert!(
            !issues
                .iter()
                .any(|i| i.code == Some(CODE_AGENT_TOOLS_FORMAT))
        );
    }

    #[test]
    fn opencode_rejects_named_color() {
        let content = "---\ncolor: red\n---\nAgent prompt";
        let issues = validate_agent_for_harness(content, HarnessKind::OpenCode);
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_AGENT_COLOR_FORMAT))
        );
    }

    #[test]
    fn opencode_accepts_hex_color() {
        let content = "---\ncolor: \"#FF5733\"\n---\nAgent prompt";
        let issues = validate_agent_for_harness(content, HarnessKind::OpenCode);
        assert!(
            !issues
                .iter()
                .any(|i| i.code == Some(CODE_AGENT_COLOR_FORMAT))
        );
    }

    #[test]
    fn claude_code_accepts_comma_string_tools() {
        let content = "---\ntools: Glob, Grep, Read\n---\nAgent prompt";
        let issues = validate_agent_for_harness(content, HarnessKind::ClaudeCode);
        assert!(
            !issues
                .iter()
                .any(|i| i.code == Some(CODE_AGENT_TOOLS_FORMAT))
        );
    }

    #[test]
    fn claude_code_accepts_named_color() {
        let content = "---\ncolor: red\n---\nAgent prompt";
        let issues = validate_agent_for_harness(content, HarnessKind::ClaudeCode);
        assert!(
            !issues
                .iter()
                .any(|i| i.code == Some(CODE_AGENT_COLOR_FORMAT))
        );
    }

    #[test]
    fn goose_returns_unsupported_error() {
        let content = "---\nname: test\n---\nAgent prompt";
        let issues = validate_agent_for_harness(content, HarnessKind::Goose);
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_AGENT_UNSUPPORTED))
        );
    }

    #[test]
    fn invalid_yaml_returns_parse_error() {
        let content = "---\ntools: [unclosed bracket\n---\nAgent prompt";
        let issues = validate_agent_for_harness(content, HarnessKind::OpenCode);
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_AGENT_PARSE_ERROR))
        );
    }

    #[test]
    fn missing_frontmatter_is_valid() {
        let content = "Just the agent prompt, no frontmatter";
        let issues = validate_agent_for_harness(content, HarnessKind::OpenCode);
        assert!(issues.is_empty());
    }

    #[test]
    fn invalid_mode_returns_error() {
        let content = "---\nmode: invalid_mode\n---\nAgent prompt";
        let issues = validate_agent_for_harness(content, HarnessKind::OpenCode);
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_AGENT_MODE_UNSUPPORTED))
        );
    }

    #[test]
    fn valid_mode_accepted() {
        let content = "---\nmode: subagent\n---\nAgent prompt";
        let issues = validate_agent_for_harness(content, HarnessKind::OpenCode);
        assert!(
            !issues
                .iter()
                .any(|i| i.code == Some(CODE_AGENT_MODE_UNSUPPORTED))
        );
    }

    #[test]
    fn is_hex_color_validates_correctly() {
        assert!(is_hex_color("#FF5733"));
        assert!(is_hex_color("#000000"));
        assert!(is_hex_color("#ffffff"));
        assert!(!is_hex_color("red"));
        assert!(!is_hex_color("#FFF"));
        assert!(!is_hex_color("FF5733"));
        assert!(!is_hex_color("#GGGGGG"));
    }

    #[test]
    fn opencode_rejects_uppercase_skill_name() {
        let content = "---\nname: Hook Development\ndescription: test\n---\nSkill content";
        let issues = validate_skill_for_harness(content, "hook-development", HarnessKind::OpenCode);
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_SKILL_NAME_FORMAT))
        );
    }

    #[test]
    fn opencode_rejects_name_directory_mismatch() {
        let content = "---\nname: other-name\ndescription: test\n---\nSkill content";
        let issues = validate_skill_for_harness(content, "actual-directory", HarnessKind::OpenCode);
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_SKILL_NAME_DIRECTORY_MISMATCH))
        );
    }

    #[test]
    fn opencode_accepts_valid_skill() {
        let content = "---\nname: my-skill\ndescription: A valid skill\n---\nSkill content";
        let issues = validate_skill_for_harness(content, "my-skill", HarnessKind::OpenCode);
        assert!(issues.is_empty());
    }

    #[test]
    fn claude_code_accepts_any_skill_name() {
        let content = "---\nname: Hook Development\ndescription: test\n---\nSkill content";
        let issues =
            validate_skill_for_harness(content, "Hook Development", HarnessKind::ClaudeCode);
        assert!(issues.is_empty());
    }

    #[test]
    fn opencode_warns_missing_description() {
        let content = "---\nname: my-skill\n---\nSkill content";
        let issues = validate_skill_for_harness(content, "my-skill", HarnessKind::OpenCode);
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_SKILL_DESCRIPTION_MISSING))
        );
        assert!(issues.iter().all(|i| i.severity == Severity::Warning));
    }

    #[test]
    fn opencode_rejects_long_skill_name() {
        let long_name = "a".repeat(65);
        let content = format!(
            "---\nname: {}\ndescription: test\n---\nSkill content",
            long_name
        );
        let issues = validate_skill_for_harness(&content, &long_name, HarnessKind::OpenCode);
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_SKILL_NAME_LENGTH))
        );
    }

    #[test]
    fn opencode_rejects_long_description() {
        let long_desc = "a".repeat(1025);
        let content = format!(
            "---\nname: my-skill\ndescription: {}\n---\nSkill content",
            long_desc
        );
        let issues = validate_skill_for_harness(&content, "my-skill", HarnessKind::OpenCode);
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_SKILL_DESCRIPTION_LENGTH))
        );
    }

    #[test]
    fn skill_name_regex_validates_correctly() {
        assert!(SKILL_NAME_RE.is_match("my-skill"));
        assert!(SKILL_NAME_RE.is_match("a"));
        assert!(SKILL_NAME_RE.is_match("skill123"));
        assert!(SKILL_NAME_RE.is_match("my-long-skill-name"));
        assert!(!SKILL_NAME_RE.is_match("My-Skill"));
        assert!(!SKILL_NAME_RE.is_match("my--skill"));
        assert!(!SKILL_NAME_RE.is_match("-my-skill"));
        assert!(!SKILL_NAME_RE.is_match("my-skill-"));
        assert!(!SKILL_NAME_RE.is_match("my skill"));
    }

    #[test]
    fn skill_capabilities_for_opencode() {
        let caps = SkillCapabilities::for_kind(HarnessKind::OpenCode).unwrap();
        assert_eq!(caps.name_format, NameFormat::LowercaseHyphenated);
        assert!(caps.name_must_match_directory);
        assert!(caps.description_required);
    }

    #[test]
    fn skill_capabilities_for_claude_code() {
        let caps = SkillCapabilities::for_kind(HarnessKind::ClaudeCode).unwrap();
        assert_eq!(caps.name_format, NameFormat::Any);
        assert!(!caps.name_must_match_directory);
        assert!(!caps.description_required);
    }

    #[test]
    fn goose_returns_skill_unsupported() {
        let content = "---\nname: test\n---\nSkill content";
        let issues = validate_skill_for_harness(content, "test", HarnessKind::Goose);
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_SKILL_UNSUPPORTED))
        );
    }

    #[test]
    fn skill_invalid_yaml_returns_parse_error() {
        let content = "---\nname: [unclosed\n---\nSkill content";
        let issues = validate_skill_for_harness(content, "test", HarnessKind::OpenCode);
        assert!(
            issues
                .iter()
                .any(|i| i.code == Some(CODE_SKILL_PARSE_ERROR))
        );
    }
}
