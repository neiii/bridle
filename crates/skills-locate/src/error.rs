//! Error types for skills discovery and fetching.

use thiserror::Error;

/// Errors that can occur during skills discovery and fetching.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    Http(String),

    /// Invalid URL provided.
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    /// GitHub reference could not be parsed.
    #[error("GitHub reference parse error: {0}")]
    GitHubParse(String),

    /// ZIP archive extraction failed.
    #[error("ZIP extraction failed: {0}")]
    ZipExtract(String),

    /// JSON parsing failed.
    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    /// YAML parsing failed.
    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    /// File not found in archive.
    #[error("file not found in archive: {0}")]
    NotFound(String),

    /// I/O operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Download size limit exceeded.
    #[error("size limit exceeded: {size} bytes > {limit} bytes")]
    SizeLimit {
        /// Actual size in bytes.
        size: u64,
        /// Maximum allowed size in bytes.
        limit: u64,
    },
}

/// A specialized Result type for skills operations.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_http() {
        let err = Error::Http("connection refused".to_string());
        assert_eq!(err.to_string(), "HTTP request failed: connection refused");
    }

    #[test]
    fn error_display_invalid_url() {
        let err = Error::InvalidUrl("not a url".to_string());
        assert_eq!(err.to_string(), "invalid URL: not a url");
    }

    #[test]
    fn error_display_github_parse() {
        let err = Error::GitHubParse("missing owner".to_string());
        assert_eq!(
            err.to_string(),
            "GitHub reference parse error: missing owner"
        );
    }

    #[test]
    fn error_display_zip_extract() {
        let err = Error::ZipExtract("corrupt archive".to_string());
        assert_eq!(err.to_string(), "ZIP extraction failed: corrupt archive");
    }

    #[test]
    fn error_display_not_found() {
        let err = Error::NotFound("config.json".to_string());
        assert_eq!(err.to_string(), "file not found in archive: config.json");
    }

    #[test]
    fn error_display_size_limit() {
        let err = Error::SizeLimit {
            size: 300_000_000,
            limit: 200_000_000,
        };
        assert_eq!(
            err.to_string(),
            "size limit exceeded: 300000000 bytes > 200000000 bytes"
        );
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }
}
