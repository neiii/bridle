//! Core type definitions for harness path resolution.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Supported AI coding harnesses.
///
/// This enum represents the different AI coding assistants whose
/// configuration paths can be discovered.
///
/// # Extensibility
///
/// This enum is marked `#[non_exhaustive]` to allow adding new
/// harness types in future versions without breaking changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum HarnessKind {
    /// Claude Code (Anthropic's CLI)
    ClaudeCode,
    /// OpenCode
    OpenCode,
    /// Goose (Block's AI coding assistant)
    Goose,
    /// AMP Code (Sourcegraph's AI coding assistant)
    AmpCode,
    /// GitHub Copilot CLI (@github/copilot npm package)
    CopilotCli,
    /// Crush (Charmbracelet's AI coding assistant)
    Crush,
    /// Factory Droid (Factory's AI coding assistant)
    Droid,
}

impl fmt::Display for HarnessKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClaudeCode => write!(f, "Claude Code"),
            Self::OpenCode => write!(f, "OpenCode"),
            Self::Goose => write!(f, "Goose"),
            Self::AmpCode => write!(f, "AMP Code"),
            Self::CopilotCli => write!(f, "Copilot CLI"),
            Self::Crush => write!(f, "Crush"),
            Self::Droid => write!(f, "Droid"),
        }
    }
}

impl HarnessKind {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::OpenCode => "OpenCode",
            Self::Goose => "Goose",
            Self::AmpCode => "AMP Code",
            Self::CopilotCli => "Copilot CLI",
            Self::Crush => "Crush",
            Self::Droid => "Droid",
        }
    }

    /// All supported harness kinds.
    ///
    /// Useful for iterating over all harnesses to check installation status
    /// or enumerate capabilities.
    ///
    /// # Examples
    ///
    /// ```
    /// use harness_locate::types::HarnessKind;
    ///
    /// for kind in HarnessKind::ALL {
    ///     println!("{}", kind);
    /// }
    /// ```
    pub const ALL: &'static [Self] = &[
        Self::ClaudeCode,
        Self::OpenCode,
        Self::Goose,
        Self::AmpCode,
        Self::CopilotCli,
        Self::Crush,
        Self::Droid,
    ];

    /// Returns the known CLI binary names for this harness.
    ///
    /// These are the executable names that indicate the harness is installed
    /// and available in PATH.
    ///
    /// # Examples
    ///
    /// ```
    /// use harness_locate::HarnessKind;
    ///
    /// assert_eq!(HarnessKind::ClaudeCode.binary_names(), &["claude"]);
    /// assert_eq!(HarnessKind::OpenCode.binary_names(), &["opencode"]);
    /// assert_eq!(HarnessKind::Goose.binary_names(), &["goose"]);
    /// ```
    #[must_use]
    pub fn binary_names(&self) -> &'static [&'static str] {
        match self {
            Self::ClaudeCode => &["claude"],
            Self::OpenCode => &["opencode"],
            Self::Goose => &["goose"],
            Self::AmpCode => &["amp"],
            Self::CopilotCli => &["copilot"],
            Self::Crush => &["crush"],
            Self::Droid => &["droid"],
        }
    }

    /// Returns the expected directory name(s) for a resource kind.
    ///
    /// Different harnesses use different naming conventions:
    /// - OpenCode uses singular names (`skill`, `command`)
    /// - Other harnesses use plural names (`skills`, `commands`)
    ///
    /// Returns `None` if the harness doesn't support that resource type.
    /// When multiple names are returned, index 0 is the canonical name.
    ///
    /// # Examples
    ///
    /// ```
    /// use harness_locate::{HarnessKind, ResourceKind};
    ///
    /// // OpenCode uses singular
    /// assert_eq!(
    ///     HarnessKind::OpenCode.directory_names(ResourceKind::Skills),
    ///     Some(&["skill"][..])
    /// );
    ///
    /// // Claude Code uses plural
    /// assert_eq!(
    ///     HarnessKind::ClaudeCode.directory_names(ResourceKind::Skills),
    ///     Some(&["skills"][..])
    /// );
    ///
    /// // Goose doesn't support commands
    /// assert_eq!(
    ///     HarnessKind::Goose.directory_names(ResourceKind::Commands),
    ///     None
    /// );
    /// ```
    #[must_use]
    pub const fn directory_names(self, resource: ResourceKind) -> Option<&'static [&'static str]> {
        match (self, resource) {
            // OpenCode - singular names
            (Self::OpenCode, ResourceKind::Skills) => Some(&["skill"]),
            (Self::OpenCode, ResourceKind::Commands) => Some(&["command"]),
            (Self::OpenCode, ResourceKind::Agents) => Some(&["agent"]),
            (Self::OpenCode, ResourceKind::Plugins) => Some(&["plugin"]),

            // Claude Code - plural names
            (Self::ClaudeCode, ResourceKind::Skills) => Some(&["skills"]),
            (Self::ClaudeCode, ResourceKind::Commands) => Some(&["commands"]),
            (Self::ClaudeCode, ResourceKind::Agents) => Some(&["agents"]),
            (Self::ClaudeCode, ResourceKind::Plugins) => Some(&["plugins"]),

            // Goose - limited support (skills only)
            (Self::Goose, ResourceKind::Skills) => Some(&["skills"]),

            // AmpCode - plural names, limited support
            (Self::AmpCode, ResourceKind::Skills) => Some(&["skills"]),
            (Self::AmpCode, ResourceKind::Commands) => Some(&["commands"]),

            // Copilot CLI - plural names, skills and agents only
            (Self::CopilotCli, ResourceKind::Skills) => Some(&["skills"]),
            (Self::CopilotCli, ResourceKind::Agents) => Some(&["agents"]),

            // Crush - skills only (like Goose)
            (Self::Crush, ResourceKind::Skills) => Some(&["skills"]),

            // Droid - plural names (like Claude Code)
            (Self::Droid, ResourceKind::Skills) => Some(&["skills"]),
            (Self::Droid, ResourceKind::Commands) => Some(&["commands"]),
            (Self::Droid, ResourceKind::Agents) => Some(&["droids"]),

            // Unsupported combinations
            _ => None,
        }
    }
}

/// Scope for path resolution.
///
/// Determines whether to look up global (user-level) or
/// project-local configuration paths.
#[derive(Debug, Clone)]
pub enum Scope {
    /// User-level global configuration (e.g., `~/.config/...`)
    Global,
    /// Project-local configuration (e.g., `.claude/` in project root)
    Project(PathBuf),
    /// Custom path for profile-scoped resources (inherits harness directory structure)
    Custom(PathBuf),
}

/// Installation status of a harness on the current system.
///
/// Represents the different states a harness can be in, from not installed
/// to fully configured with both binary and configuration present.
///
/// # Extensibility
///
/// This enum is marked `#[non_exhaustive]` to allow adding new
/// status variants in future versions without breaking changes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InstallationStatus {
    /// Harness is not installed (no binary or config found).
    NotInstalled,
    /// Only configuration directory exists (no binary in PATH).
    ConfigOnly {
        /// Path to the configuration directory.
        config_path: PathBuf,
    },
    /// Only the binary exists in PATH (no configuration found).
    BinaryOnly {
        /// Path to the binary executable.
        binary_path: PathBuf,
    },
    /// Fully installed with both binary and configuration.
    FullyInstalled {
        /// Path to the binary executable.
        binary_path: PathBuf,
        /// Path to the configuration directory.
        config_path: PathBuf,
    },
}

impl InstallationStatus {
    /// Returns `true` if the harness CLI can be invoked.
    ///
    /// A harness is runnable if its binary is available in PATH,
    /// regardless of whether configuration exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use harness_locate::InstallationStatus;
    /// use std::path::PathBuf;
    ///
    /// let status = InstallationStatus::BinaryOnly {
    ///     binary_path: PathBuf::from("/usr/bin/claude"),
    /// };
    /// assert!(status.is_runnable());
    ///
    /// let status = InstallationStatus::NotInstalled;
    /// assert!(!status.is_runnable());
    /// ```
    #[must_use]
    pub fn is_runnable(&self) -> bool {
        matches!(self, Self::BinaryOnly { .. } | Self::FullyInstalled { .. })
    }

    /// Returns the binary path if available.
    ///
    /// # Examples
    ///
    /// ```
    /// use harness_locate::InstallationStatus;
    /// use std::path::{Path, PathBuf};
    ///
    /// let status = InstallationStatus::FullyInstalled {
    ///     binary_path: PathBuf::from("/usr/bin/claude"),
    ///     config_path: PathBuf::from("/home/user/.claude"),
    /// };
    /// assert_eq!(status.binary_path(), Some(Path::new("/usr/bin/claude")));
    /// ```
    #[must_use]
    pub fn binary_path(&self) -> Option<&Path> {
        match self {
            Self::BinaryOnly { binary_path } | Self::FullyInstalled { binary_path, .. } => {
                Some(binary_path)
            }
            _ => None,
        }
    }

    /// Returns the config path if available.
    ///
    /// # Examples
    ///
    /// ```
    /// use harness_locate::InstallationStatus;
    /// use std::path::{Path, PathBuf};
    ///
    /// let status = InstallationStatus::ConfigOnly {
    ///     config_path: PathBuf::from("/home/user/.claude"),
    /// };
    /// assert_eq!(status.config_path(), Some(Path::new("/home/user/.claude")));
    /// ```
    #[must_use]
    pub fn config_path(&self) -> Option<&Path> {
        match self {
            Self::ConfigOnly { config_path } | Self::FullyInstalled { config_path, .. } => {
                Some(config_path)
            }
            _ => None,
        }
    }
}

/// Types of paths a harness may provide.
///
/// Each harness can have different configuration directories
/// for different purposes.
///
/// # Extensibility
///
/// This enum is marked `#[non_exhaustive]` to allow adding new
/// path types in future versions without breaking changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathType {
    /// Main configuration directory
    Config,
    /// Skills/capabilities definitions
    Skills,
    /// Custom commands
    Commands,
    /// MCP (Model Context Protocol) configuration
    Mcp,
    /// Rules and constraints
    Rules,
}

/// Categories of resources that harnesses manage in named directories.
///
/// Used with [`HarnessKind::directory_names`] to query expected
/// directory naming conventions.
///
/// **Note:** Rules are not included because they are stored at the root
/// level (config dir or project root), not in a named subdirectory.
///
/// # Extensibility
///
/// This enum is marked `#[non_exhaustive]` to allow adding new
/// resource kinds in future versions without breaking changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResourceKind {
    /// Skills/capabilities definitions
    Skills,
    /// Custom commands
    Commands,
    /// Agent definitions
    Agents,
    /// Plugin extensions
    Plugins,
}

/// File formats used by harness configuration files.
///
/// Different harnesses use different formats for their configuration,
/// commands, and other resources.
///
/// # Extensibility
///
/// This enum is marked `#[non_exhaustive]` to allow adding new
/// formats in future versions without breaking changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FileFormat {
    /// Standard JSON format.
    Json,
    /// JSON with comments (JSONC).
    Jsonc,
    /// YAML format.
    Yaml,
    /// Plain Markdown.
    Markdown,
    /// Markdown with YAML frontmatter.
    MarkdownWithFrontmatter,
}

/// Directory layout structure for resource directories.
///
/// Harnesses organize their resources in different ways:
/// - Flat: Files directly in the directory (e.g., `commands/foo.md`)
/// - Nested: Subdirectory per resource (e.g., `skills/foo/SKILL.md`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DirectoryStructure {
    /// Files directly in the directory.
    ///
    /// Example: `commands/foo.md`, `commands/bar.md`
    Flat {
        /// Glob pattern for matching files (e.g., `"*.md"`).
        file_pattern: String,
    },
    /// Subdirectory per resource with a fixed filename inside.
    ///
    /// Example: `skills/foo/SKILL.md`, `skills/bar/SKILL.md`
    Nested {
        /// Pattern for subdirectory names (e.g., `"*"`).
        subdir_pattern: String,
        /// Fixed filename or marker directory within each subdirectory.
        ///
        /// Can be a file (e.g., `"SKILL.md"`) or a marker directory
        /// (e.g., `".claude-plugin"` for plugin detection).
        file_name: String,
    },
}

/// A directory-based resource location.
///
/// Represents a directory that contains multiple resource files,
/// such as commands or skills directories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryResource {
    /// Path to the directory.
    pub path: PathBuf,
    /// Whether the directory currently exists on the filesystem.
    pub exists: bool,
    /// How resources are organized within the directory.
    pub structure: DirectoryStructure,
    /// Format of files within the directory.
    pub file_format: FileFormat,
}

/// A configuration file resource location.
///
/// Represents a single configuration file that may contain
/// multiple configuration entries, accessed via a key path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResource {
    /// Path to the configuration file.
    pub file: PathBuf,
    /// Whether the file currently exists on the filesystem.
    pub file_exists: bool,
    /// JSON pointer path to the relevant section (e.g., `"/mcpServers"`).
    pub key_path: String,
    /// Format of the configuration file.
    pub format: FileFormat,
    /// Optional JSON Schema URL for validation.
    pub schema_url: Option<String>,
}

/// A value that may be a plain string or a reference to an environment variable.
///
/// This type handles the different syntax each harness uses for environment
/// variable references:
/// - Claude Code: `${VAR}`
/// - OpenCode: `{env:VAR}`
/// - Goose: Uses `env_keys` array, values resolved at runtime
///
/// # Serde Behavior
///
/// Uses `#[serde(untagged)]` for clean JSON representation:
/// - Plain string: `"hello"` deserializes to `Plain("hello")`
/// - Object with env key: `{"env": "VAR"}` deserializes to `EnvRef { env: "VAR" }`
///
/// # Examples
///
/// ```
/// use harness_locate::types::{EnvValue, HarnessKind};
///
/// // Create an environment variable reference
/// let api_key = EnvValue::env("MY_API_KEY");
///
/// // Convert to Claude Code format
/// assert_eq!(api_key.to_native(HarnessKind::ClaudeCode), "${MY_API_KEY}");
///
/// // Convert to OpenCode format
/// assert_eq!(api_key.to_native(HarnessKind::OpenCode), "{env:MY_API_KEY}");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnvValue {
    /// A plain string value.
    Plain(String),
    /// A reference to an environment variable.
    EnvRef {
        /// The name of the environment variable.
        env: String,
    },
}

impl EnvValue {
    /// Creates a plain string value.
    ///
    /// # Examples
    ///
    /// ```
    /// use harness_locate::types::EnvValue;
    ///
    /// let value = EnvValue::plain("hello");
    /// assert_eq!(value.resolve(), Some("hello".to_string()));
    /// ```
    #[must_use]
    pub fn plain(s: impl Into<String>) -> Self {
        Self::Plain(s.into())
    }

    /// Creates an environment variable reference.
    ///
    /// # Examples
    ///
    /// ```
    /// use harness_locate::types::EnvValue;
    ///
    /// let value = EnvValue::env("MY_VAR");
    /// // Resolution depends on whether MY_VAR is set in the environment
    /// ```
    #[must_use]
    pub fn env(var: impl Into<String>) -> Self {
        Self::EnvRef { env: var.into() }
    }

    /// Converts to the harness-specific native string format.
    ///
    /// # Arguments
    ///
    /// * `kind` - The target harness format
    ///
    /// # Returns
    ///
    /// - For `Plain`: Returns the string as-is
    /// - For `EnvRef` with Claude Code: Returns `${VAR}`
    /// - For `EnvRef` with OpenCode: Returns `{env:VAR}`
    /// - For `EnvRef` with Goose: Resolves the env var immediately
    ///
    /// # Examples
    ///
    /// ```
    /// use harness_locate::types::{EnvValue, HarnessKind};
    ///
    /// let value = EnvValue::env("API_KEY");
    /// assert_eq!(value.to_native(HarnessKind::ClaudeCode), "${API_KEY}");
    /// assert_eq!(value.to_native(HarnessKind::OpenCode), "{env:API_KEY}");
    /// ```
    #[must_use]
    pub fn to_native(&self, kind: HarnessKind) -> String {
        match self {
            Self::Plain(s) => s.clone(),
            Self::EnvRef { env } => match kind {
                HarnessKind::ClaudeCode
                | HarnessKind::AmpCode
                | HarnessKind::CopilotCli
                | HarnessKind::Droid => {
                    format!("${{{env}}}")
                }
                HarnessKind::OpenCode | HarnessKind::Crush => format!("{{env:{env}}}"),
                HarnessKind::Goose => std::env::var(env).unwrap_or_default(),
            },
        }
    }

    /// Fallible version of [`to_native`](Self::to_native) that returns an error
    /// when an environment variable reference cannot be resolved.
    ///
    /// For Goose harness, this validates that the referenced environment variable
    /// is actually set, returning `Error::MissingEnvVar` if not.
    ///
    /// For other harnesses that use template syntax (Claude Code, OpenCode, AmpCode),
    /// this behaves identically to `to_native` since the variable is not resolved
    /// at conversion time.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::MissingEnvVar`] if the harness is Goose and the
    /// referenced environment variable is not set.
    ///
    /// # Examples
    ///
    /// ```
    /// use harness_locate::types::{EnvValue, HarnessKind};
    ///
    /// // Template-based harnesses always succeed
    /// let value = EnvValue::env("SOME_VAR");
    /// assert!(value.try_to_native(HarnessKind::ClaudeCode).is_ok());
    ///
    /// // Goose requires the env var to be set
    /// // SAFETY: Test environment only, no concurrent access
    /// unsafe { std::env::set_var("TEST_VAR", "value"); }
    /// let value = EnvValue::env("TEST_VAR");
    /// assert_eq!(value.try_to_native(HarnessKind::Goose).unwrap(), "value");
    /// ```
    pub fn try_to_native(&self, kind: HarnessKind) -> crate::Result<String> {
        match self {
            Self::Plain(s) => Ok(s.clone()),
            Self::EnvRef { env } => match kind {
                HarnessKind::ClaudeCode
                | HarnessKind::AmpCode
                | HarnessKind::CopilotCli
                | HarnessKind::Droid => Ok(format!("${{{env}}}")),
                HarnessKind::OpenCode | HarnessKind::Crush => Ok(format!("{{env:{env}}}")),
                HarnessKind::Goose => std::env::var(env)
                    .map_err(|_| crate::Error::MissingEnvVar { name: env.clone() }),
            },
        }
    }

    /// Parses a harness-specific native string format into an `EnvValue`.
    ///
    /// # Arguments
    ///
    /// * `s` - The string to parse
    /// * `kind` - The source harness format
    ///
    /// # Returns
    ///
    /// - For Claude Code: Parses `${VAR}` pattern
    /// - For OpenCode: Parses `{env:VAR}` pattern
    /// - For Goose: Always returns `Plain` (Goose doesn't use inline syntax)
    /// - If no pattern matches, returns `Plain`
    ///
    /// # Examples
    ///
    /// ```
    /// use harness_locate::types::{EnvValue, HarnessKind};
    ///
    /// let value = EnvValue::from_native("${API_KEY}", HarnessKind::ClaudeCode);
    /// assert_eq!(value, EnvValue::env("API_KEY"));
    ///
    /// let value = EnvValue::from_native("{env:API_KEY}", HarnessKind::OpenCode);
    /// assert_eq!(value, EnvValue::env("API_KEY"));
    ///
    /// let value = EnvValue::from_native("plain text", HarnessKind::ClaudeCode);
    /// assert_eq!(value, EnvValue::plain("plain text"));
    /// ```
    #[must_use]
    pub fn from_native(s: &str, kind: HarnessKind) -> Self {
        match kind {
            HarnessKind::ClaudeCode
            | HarnessKind::AmpCode
            | HarnessKind::CopilotCli
            | HarnessKind::Droid => {
                if let Some(var) = s.strip_prefix("${").and_then(|s| s.strip_suffix('}')) {
                    Self::EnvRef {
                        env: var.to_string(),
                    }
                } else {
                    Self::Plain(s.to_string())
                }
            }
            HarnessKind::OpenCode | HarnessKind::Crush => {
                if let Some(var) = s.strip_prefix("{env:").and_then(|s| s.strip_suffix('}')) {
                    Self::EnvRef {
                        env: var.to_string(),
                    }
                } else {
                    Self::Plain(s.to_string())
                }
            }
            HarnessKind::Goose => Self::Plain(s.to_string()),
        }
    }

    /// Resolves the value, looking up environment variables if needed.
    ///
    /// # Returns
    ///
    /// - For `Plain`: Returns `Some(value)`
    /// - For `EnvRef`: Returns `Some(value)` if the env var is set, `None` otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// use harness_locate::types::EnvValue;
    ///
    /// let plain = EnvValue::plain("hello");
    /// assert_eq!(plain.resolve(), Some("hello".to_string()));
    ///
    /// // EnvRef resolution depends on whether the variable is set
    /// let env_ref = EnvValue::env("UNLIKELY_TO_EXIST_12345");
    /// assert_eq!(env_ref.resolve(), None);
    /// ```
    #[must_use]
    pub fn resolve(&self) -> Option<String> {
        match self {
            Self::Plain(s) => Some(s.clone()),
            Self::EnvRef { env } => std::env::var(env).ok(),
        }
    }

    /// Returns `true` if this is a plain string value.
    #[must_use]
    pub fn is_plain(&self) -> bool {
        matches!(self, Self::Plain(_))
    }

    /// Returns `true` if this is an environment variable reference.
    #[must_use]
    pub fn is_env_ref(&self) -> bool {
        matches!(self, Self::EnvRef { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_constructor() {
        let value = EnvValue::plain("hello");
        assert!(value.is_plain());
        assert!(!value.is_env_ref());
        assert_eq!(value, EnvValue::Plain("hello".to_string()));
    }

    #[test]
    fn env_constructor() {
        let value = EnvValue::env("MY_VAR");
        assert!(value.is_env_ref());
        assert!(!value.is_plain());
        assert_eq!(
            value,
            EnvValue::EnvRef {
                env: "MY_VAR".to_string()
            }
        );
    }

    #[test]
    fn to_native_plain_returns_value_unchanged() {
        let value = EnvValue::plain("hello world");
        assert_eq!(value.to_native(HarnessKind::ClaudeCode), "hello world");
        assert_eq!(value.to_native(HarnessKind::OpenCode), "hello world");
        assert_eq!(value.to_native(HarnessKind::Goose), "hello world");
    }

    #[test]
    fn to_native_claude_code_format() {
        let value = EnvValue::env("API_KEY");
        assert_eq!(value.to_native(HarnessKind::ClaudeCode), "${API_KEY}");
    }

    #[test]
    fn to_native_opencode_format() {
        let value = EnvValue::env("API_KEY");
        assert_eq!(value.to_native(HarnessKind::OpenCode), "{env:API_KEY}");
    }

    #[test]
    fn to_native_goose_resolves_env_var() {
        // SAFETY: Test runs single-threaded; no concurrent access to this env var
        unsafe { std::env::set_var("TEST_GOOSE_VAR", "resolved_value") };
        let value = EnvValue::env("TEST_GOOSE_VAR");
        assert_eq!(value.to_native(HarnessKind::Goose), "resolved_value");
        unsafe { std::env::remove_var("TEST_GOOSE_VAR") };
    }

    #[test]
    fn to_native_goose_returns_empty_for_unset_var() {
        let value = EnvValue::env("UNLIKELY_VAR_NAME_12345");
        assert_eq!(value.to_native(HarnessKind::Goose), "");
    }

    #[test]
    fn try_to_native_plain_always_succeeds() {
        let value = EnvValue::plain("hello world");
        assert_eq!(
            value.try_to_native(HarnessKind::ClaudeCode).unwrap(),
            "hello world"
        );
        assert_eq!(
            value.try_to_native(HarnessKind::OpenCode).unwrap(),
            "hello world"
        );
        assert_eq!(
            value.try_to_native(HarnessKind::Goose).unwrap(),
            "hello world"
        );
    }

    #[test]
    fn try_to_native_template_harnesses_always_succeed() {
        let value = EnvValue::env("NONEXISTENT_VAR_XYZ");
        assert_eq!(
            value.try_to_native(HarnessKind::ClaudeCode).unwrap(),
            "${NONEXISTENT_VAR_XYZ}"
        );
        assert_eq!(
            value.try_to_native(HarnessKind::OpenCode).unwrap(),
            "{env:NONEXISTENT_VAR_XYZ}"
        );
        assert_eq!(
            value.try_to_native(HarnessKind::AmpCode).unwrap(),
            "${NONEXISTENT_VAR_XYZ}"
        );
    }

    #[test]
    fn try_to_native_goose_succeeds_when_var_set() {
        unsafe { std::env::set_var("TEST_TRY_NATIVE_VAR", "success") };
        let value = EnvValue::env("TEST_TRY_NATIVE_VAR");
        assert_eq!(value.try_to_native(HarnessKind::Goose).unwrap(), "success");
        unsafe { std::env::remove_var("TEST_TRY_NATIVE_VAR") };
    }

    #[test]
    fn try_to_native_goose_fails_when_var_unset() {
        let value = EnvValue::env("DEFINITELY_NOT_SET_VAR_ABC");
        let result = value.try_to_native(HarnessKind::Goose);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, crate::Error::MissingEnvVar { name } if name == "DEFINITELY_NOT_SET_VAR_ABC")
        );
    }

    #[test]
    fn from_native_claude_code_parses_env_ref() {
        let value = EnvValue::from_native("${MY_VAR}", HarnessKind::ClaudeCode);
        assert_eq!(value, EnvValue::env("MY_VAR"));
    }

    #[test]
    fn from_native_claude_code_plain_for_non_matching() {
        let value = EnvValue::from_native("plain text", HarnessKind::ClaudeCode);
        assert_eq!(value, EnvValue::plain("plain text"));
    }

    #[test]
    fn from_native_opencode_parses_env_ref() {
        let value = EnvValue::from_native("{env:MY_VAR}", HarnessKind::OpenCode);
        assert_eq!(value, EnvValue::env("MY_VAR"));
    }

    #[test]
    fn from_native_opencode_plain_for_non_matching() {
        let value = EnvValue::from_native("plain text", HarnessKind::OpenCode);
        assert_eq!(value, EnvValue::plain("plain text"));
    }

    #[test]
    fn from_native_goose_always_plain() {
        let value = EnvValue::from_native("${MY_VAR}", HarnessKind::Goose);
        assert_eq!(value, EnvValue::plain("${MY_VAR}"));

        let value = EnvValue::from_native("{env:MY_VAR}", HarnessKind::Goose);
        assert_eq!(value, EnvValue::plain("{env:MY_VAR}"));
    }

    #[test]
    fn resolve_plain_returns_value() {
        let value = EnvValue::plain("hello");
        assert_eq!(value.resolve(), Some("hello".to_string()));
    }

    #[test]
    fn resolve_env_ref_returns_value_when_set() {
        // SAFETY: Test runs single-threaded; no concurrent access to this env var
        unsafe { std::env::set_var("TEST_RESOLVE_VAR", "test_value") };
        let value = EnvValue::env("TEST_RESOLVE_VAR");
        assert_eq!(value.resolve(), Some("test_value".to_string()));
        unsafe { std::env::remove_var("TEST_RESOLVE_VAR") };
    }

    #[test]
    fn resolve_env_ref_returns_none_when_unset() {
        let value = EnvValue::env("UNLIKELY_VAR_NAME_67890");
        assert_eq!(value.resolve(), None);
    }

    #[test]
    fn serde_plain_string_roundtrip() {
        let value = EnvValue::plain("hello");
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, r#""hello""#);
        let parsed: EnvValue = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn serde_env_ref_roundtrip() {
        let value = EnvValue::env("MY_VAR");
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, r#"{"env":"MY_VAR"}"#);
        let parsed: EnvValue = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn serde_deserialize_plain_from_string() {
        let parsed: EnvValue = serde_json::from_str(r#""plain text""#).unwrap();
        assert_eq!(parsed, EnvValue::plain("plain text"));
    }

    #[test]
    fn serde_deserialize_env_ref_from_object() {
        let parsed: EnvValue = serde_json::from_str(r#"{"env":"API_KEY"}"#).unwrap();
        assert_eq!(parsed, EnvValue::env("API_KEY"));
    }

    #[test]
    fn binary_names_claude_code() {
        assert_eq!(HarnessKind::ClaudeCode.binary_names(), &["claude"]);
    }

    #[test]
    fn binary_names_opencode() {
        assert_eq!(HarnessKind::OpenCode.binary_names(), &["opencode"]);
    }

    #[test]
    fn binary_names_goose() {
        assert_eq!(HarnessKind::Goose.binary_names(), &["goose"]);
    }

    #[test]
    fn binary_names_returns_static_slice() {
        for kind in HarnessKind::ALL {
            assert_eq!(kind.binary_names().len(), 1);
        }
    }

    #[test]
    fn installation_status_is_runnable() {
        assert!(!InstallationStatus::NotInstalled.is_runnable());
        assert!(
            !InstallationStatus::ConfigOnly {
                config_path: PathBuf::from("/config"),
            }
            .is_runnable()
        );
        assert!(
            InstallationStatus::BinaryOnly {
                binary_path: PathBuf::from("/bin"),
            }
            .is_runnable()
        );
        assert!(
            InstallationStatus::FullyInstalled {
                binary_path: PathBuf::from("/bin"),
                config_path: PathBuf::from("/config"),
            }
            .is_runnable()
        );
    }

    #[test]
    fn installation_status_accessors() {
        let status = InstallationStatus::FullyInstalled {
            binary_path: PathBuf::from("/bin/claude"),
            config_path: PathBuf::from("/home/.claude"),
        };
        assert_eq!(status.binary_path(), Some(Path::new("/bin/claude")));
        assert_eq!(status.config_path(), Some(Path::new("/home/.claude")));

        let status = InstallationStatus::NotInstalled;
        assert_eq!(status.binary_path(), None);
        assert_eq!(status.config_path(), None);
    }

    #[test]
    fn directory_names_opencode_singular() {
        assert_eq!(
            HarnessKind::OpenCode.directory_names(ResourceKind::Skills),
            Some(&["skill"][..])
        );
        assert_eq!(
            HarnessKind::OpenCode.directory_names(ResourceKind::Commands),
            Some(&["command"][..])
        );
        assert_eq!(
            HarnessKind::OpenCode.directory_names(ResourceKind::Agents),
            Some(&["agent"][..])
        );
        assert_eq!(
            HarnessKind::OpenCode.directory_names(ResourceKind::Plugins),
            Some(&["plugin"][..])
        );
    }

    #[test]
    fn directory_names_claude_code_plural() {
        assert_eq!(
            HarnessKind::ClaudeCode.directory_names(ResourceKind::Skills),
            Some(&["skills"][..])
        );
        assert_eq!(
            HarnessKind::ClaudeCode.directory_names(ResourceKind::Commands),
            Some(&["commands"][..])
        );
    }

    #[test]
    fn directory_names_unsupported_returns_none() {
        assert_eq!(
            HarnessKind::Goose.directory_names(ResourceKind::Commands),
            None
        );
        assert_eq!(
            HarnessKind::Goose.directory_names(ResourceKind::Plugins),
            None
        );
    }

    #[test]
    fn directory_names_all_harnesses_support_skills() {
        for kind in HarnessKind::ALL {
            assert!(
                kind.directory_names(ResourceKind::Skills).is_some(),
                "{kind} should support skills"
            );
        }
    }
}
