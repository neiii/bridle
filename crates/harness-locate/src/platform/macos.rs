//! macOS-specific path resolution.

use std::path::PathBuf;

use crate::error::Result;

/// Returns the user's config directory on macOS.
///
/// Most CLI tools use `~/.config/` following XDG conventions,
/// though native macOS apps prefer `~/Library/Application Support/`.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn config_dir() -> Result<PathBuf> {
    Ok(super::home_dir()?.join(".config"))
}

/// Returns the user's data directory on macOS.
///
/// Returns `~/Library/Application Support/` for native macOS conventions.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn data_dir() -> Result<PathBuf> {
    Ok(super::home_dir()?.join("Library/Application Support"))
}

/// Returns the Application Support directory on macOS.
///
/// This is `~/Library/Application Support/`, used by native macOS applications.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn application_support_dir() -> Result<PathBuf> {
    Ok(super::home_dir()?.join("Library/Application Support"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_is_dot_config() {
        let result = config_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(
            path.ends_with(".config"),
            "config_dir should end with .config"
        );
    }

    #[test]
    fn data_dir_is_application_support() {
        let result = data_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(
            path.ends_with("Library/Application Support"),
            "data_dir should end with Library/Application Support"
        );
    }

    #[test]
    fn application_support_dir_matches_data_dir() {
        let app_support = application_support_dir().unwrap();
        let data = data_dir().unwrap();
        assert_eq!(app_support, data);
    }
}
