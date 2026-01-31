//! Binary detection utilities.
//!
//! This module provides cross-platform binary detection using the `which` crate.

use std::path::PathBuf;

use crate::error::{Error, Result};

/// Finds a binary executable in PATH.
///
/// Returns `Ok(Some(path))` if found, `Ok(None)` if not found,
/// or `Err` for system errors (e.g., canonicalization failures).
///
/// Cross-platform: handles Windows extensions (.exe, .cmd, etc.) automatically.
///
/// # Arguments
///
/// * `name` - The binary name to search for (without extension on Windows)
///
/// # Errors
///
/// Returns `Error::BinaryDetection` if a system error occurs during search.
///
/// # Examples
///
/// ```no_run
/// use harness_locate::detection::find_binary;
///
/// match find_binary("claude") {
///     Ok(Some(path)) => println!("Found at: {}", path.display()),
///     Ok(None) => println!("Not installed"),
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
pub fn find_binary(name: &str) -> Result<Option<PathBuf>> {
    match which::which(name) {
        Ok(path) => Ok(Some(path)),
        Err(which::Error::CannotFindBinaryPath) => Ok(None),
        Err(e) => Err(Error::BinaryDetection(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_binary_returns_none_for_nonexistent() {
        let result = find_binary("nonexistent-binary-xyz-12345");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn find_binary_returns_some_for_common_binary() {
        #[cfg(unix)]
        let binary = "ls";
        #[cfg(windows)]
        let binary = "cmd";

        let result = find_binary(binary);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
}
