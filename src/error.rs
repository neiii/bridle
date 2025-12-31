//! Error types for bridle CLI.

#![allow(dead_code)]

use thiserror::Error;

/// Result type alias using bridle's Error.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in bridle.
#[derive(Error, Debug)]
pub enum Error {
    /// No configuration file found at expected location.
    #[error("no config found: {0}")]
    NoConfigFound(String),

    /// Failed to read or write configuration.
    #[error("config error: {0}")]
    Config(String),

    /// Harness executable not installed or not in PATH.
    #[error("harness not installed")]
    HarnessNotInstalled,

    /// Profile with given name does not exist.
    #[error("profile not found: {0}")]
    ProfileNotFound(String),

    /// Profile with given name already exists.
    #[error("profile already exists: {0}")]
    ProfileExists(String),

    /// No profile is currently active.
    #[error("no active profile")]
    NoActiveProfile,

    /// Profile name contains invalid characters.
    #[error("invalid profile name: {0}")]
    InvalidProfileName(String),

    /// Unknown harness name.
    #[error("unknown harness: {0}\nValid options: claude-code, opencode, goose, amp-code")]
    UnknownHarness(String),

    /// Command failed.
    #[error("{0}")]
    Command(String),

    /// Unknown configuration setting.
    #[error("unknown setting: {0}\nValid options: editor, marker_files, default_harness")]
    UnknownSetting(String),

    /// Invalid configuration value.
    #[error("invalid value: {0}")]
    InvalidValue(String),

    /// IO error.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// TOML parsing error.
    #[error(transparent)]
    Toml(#[from] toml::de::Error),

    /// JSON error.
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// Harness error.
    #[error(transparent)]
    Harness(#[from] harness_locate::Error),

    /// YAML parsing error.
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
}
