//! Platform-specific path resolution.
//!
//! This module provides functions to resolve base configuration directories
//! on each supported platform (macOS, Linux, Windows).

use std::path::PathBuf;

use crate::error::{Error, Result};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

/// Returns the user's home directory.
///
/// # Errors
///
/// Returns [`Error::NotFound`] if the home directory cannot be determined.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub fn home_dir() -> Result<PathBuf> {
    home::home_dir().ok_or_else(|| Error::NotFound("home directory".into()))
}

/// Returns the user's home directory.
///
/// # Errors
///
/// Returns [`Error::UnsupportedPlatform`] on unsupported platforms.
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn home_dir() -> Result<PathBuf> {
    Err(Error::UnsupportedPlatform)
}

/// Returns the user's config directory.
///
/// Platform-specific behavior:
/// - **macOS**: `~/.config/`
/// - **Linux**: `$XDG_CONFIG_HOME` or `~/.config/`
/// - **Windows**: `%APPDATA%`
///
/// # Errors
///
/// Returns an error if the config directory cannot be determined.
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn config_dir() -> Result<PathBuf> {
    Err(Error::UnsupportedPlatform)
}

/// Returns the user's data directory.
///
/// Platform-specific behavior:
/// - **macOS**: `~/Library/Application Support/`
/// - **Linux**: `$XDG_DATA_HOME` or `~/.local/share/`
/// - **Windows**: `%LOCALAPPDATA%`
///
/// # Errors
///
/// Returns an error if the data directory cannot be determined.
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn data_dir() -> Result<PathBuf> {
    Err(Error::UnsupportedPlatform)
}

#[cfg(all(test, any(target_os = "linux", target_os = "windows")))]
pub(crate) mod test_utils {
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    pub struct EnvGuard<'a> {
        _lock: std::sync::MutexGuard<'a, ()>,
        vars: Vec<(String, Option<String>)>,
    }

    impl<'a> EnvGuard<'a> {
        pub fn new() -> Self {
            Self {
                _lock: ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner()),
                vars: Vec::new(),
            }
        }

        pub fn set(&mut self, key: &str, value: &str) {
            let original = std::env::var(key).ok();
            if !self.vars.iter().any(|(k, _)| k == key) {
                self.vars.push((key.to_string(), original));
            }
            // SAFETY: We hold ENV_LOCK ensuring single-threaded env access in tests
            unsafe { std::env::set_var(key, value) };
        }

        pub fn remove(&mut self, key: &str) {
            let original = std::env::var(key).ok();
            if !self.vars.iter().any(|(k, _)| k == key) {
                self.vars.push((key.to_string(), original));
            }
            // SAFETY: We hold ENV_LOCK ensuring single-threaded env access in tests
            unsafe { std::env::remove_var(key) };
        }
    }

    impl Drop for EnvGuard<'_> {
        fn drop(&mut self) {
            for (key, original) in &self.vars {
                // SAFETY: We hold ENV_LOCK ensuring single-threaded env access in tests
                match original {
                    Some(val) => unsafe { std::env::set_var(key, val) },
                    None => unsafe { std::env::remove_var(key) },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    use super::test_utils::EnvGuard;
    use super::*;

    #[test]
    fn home_dir_exists() {
        let result = home_dir();
        assert!(result.is_ok(), "home_dir should succeed");
        let path = result.unwrap();
        assert!(
            path.is_absolute(),
            "home_dir should return an absolute path"
        );
    }

    #[test]
    fn config_dir_exists() {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        let _env = EnvGuard::new();

        let result = config_dir();
        assert!(result.is_ok(), "config_dir should succeed");
        let path = result.unwrap();
        assert!(
            path.is_absolute(),
            "config_dir should return an absolute path"
        );
    }

    #[test]
    fn data_dir_exists() {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        let _env = EnvGuard::new();

        let result = data_dir();
        assert!(result.is_ok(), "data_dir should succeed");
        let path = result.unwrap();
        assert!(
            path.is_absolute(),
            "data_dir should return an absolute path"
        );
    }
}
