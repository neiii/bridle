//! Windows-specific path resolution.

use std::path::PathBuf;

use crate::error::{Error, Result};

/// Returns the user's config directory on Windows.
///
/// Returns `%APPDATA%` which is typically `C:\Users\<user>\AppData\Roaming`.
///
/// # Errors
///
/// Returns an error if the `APPDATA` environment variable is not set.
pub fn config_dir() -> Result<PathBuf> {
    std::env::var("APPDATA")
        .map(PathBuf::from)
        .map_err(Error::from)
}

/// Returns the user's data directory on Windows.
///
/// Returns `%LOCALAPPDATA%` which is typically `C:\Users\<user>\AppData\Local`.
///
/// # Errors
///
/// Returns an error if the `LOCALAPPDATA` environment variable is not set.
pub fn data_dir() -> Result<PathBuf> {
    std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .map_err(Error::from)
}

/// Returns the roaming app data directory on Windows.
///
/// This is `%APPDATA%`, used for settings that should roam with the user.
///
/// # Errors
///
/// Returns an error if the `APPDATA` environment variable is not set.
pub fn roaming_app_data_dir() -> Result<PathBuf> {
    config_dir()
}

/// Returns the local app data directory on Windows.
///
/// This is `%LOCALAPPDATA%`, used for data that should stay on the local machine.
///
/// # Errors
///
/// Returns an error if the `LOCALAPPDATA` environment variable is not set.
pub fn local_app_data_dir() -> Result<PathBuf> {
    data_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::test_utils::EnvGuard;

    #[test]
    fn config_dir_uses_appdata() {
        let mut env = EnvGuard::new();
        env.set("APPDATA", r"C:\Users\Test\AppData\Roaming");

        let result = config_dir();
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            PathBuf::from(r"C:\Users\Test\AppData\Roaming")
        );
    }

    #[test]
    fn data_dir_uses_localappdata() {
        let mut env = EnvGuard::new();
        env.set("LOCALAPPDATA", r"C:\Users\Test\AppData\Local");

        let result = data_dir();
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            PathBuf::from(r"C:\Users\Test\AppData\Local")
        );
    }

    #[test]
    fn missing_appdata_returns_error() {
        let mut env = EnvGuard::new();
        env.remove("APPDATA");

        let result = config_dir();
        assert!(result.is_err());
    }
}
