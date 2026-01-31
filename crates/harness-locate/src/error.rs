//! Error types for harness operations.

use std::path::PathBuf;

/// Errors that can occur during harness operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The requested harness was not found on this system.
    #[error("harness not found: {0}")]
    NotFound(String),

    /// The path is invalid or inaccessible.
    #[error("invalid path: {0}")]
    InvalidPath(PathBuf),

    /// An environment variable could not be read.
    #[error("environment variable error: {0}")]
    EnvVar(#[from] std::env::VarError),

    /// The current platform is not supported.
    #[error("unsupported platform")]
    UnsupportedPlatform,

    /// An I/O error occurred.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// MCP server uses unsupported features for target harness.
    #[error("unsupported MCP config for {harness}: {reason}")]
    UnsupportedMcpConfig {
        /// The harness that doesn't support the config.
        harness: String,
        /// Explanation of what's unsupported.
        reason: String,
    },

    /// Binary detection failed due to system error.
    #[error("binary detection error: {0}")]
    BinaryDetection(String),

    /// The requested scope is not supported by this harness.
    #[error("{harness} does not support {scope} scope")]
    UnsupportedScope { harness: String, scope: String },

    /// YAML parsing failed.
    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    /// A required field is missing from the input.
    #[error("missing required field: {0}")]
    MissingField(String),

    /// An environment variable referenced by EnvValue is not set.
    #[error("missing environment variable: {name}")]
    MissingEnvVar {
        /// The name of the environment variable that was not set.
        name: String,
    },
}

/// A specialized Result type for harness operations.
pub type Result<T> = std::result::Result<T, Error>;
