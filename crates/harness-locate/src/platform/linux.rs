//! Linux-specific path resolution with XDG Base Directory support.

use std::path::PathBuf;

use crate::error::Result;

fn xdg_path_if_valid(var_name: &str) -> Option<PathBuf> {
    std::env::var(var_name).ok().and_then(|val| {
        let path = PathBuf::from(&val);
        if !val.is_empty() && path.is_absolute() {
            Some(path)
        } else {
            None
        }
    })
}

/// Returns the user's config directory on Linux.
///
/// Respects `XDG_CONFIG_HOME` if set to an absolute path,
/// otherwise defaults to `~/.config/`.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn config_dir() -> Result<PathBuf> {
    if let Some(path) = xdg_path_if_valid("XDG_CONFIG_HOME") {
        return Ok(path);
    }
    Ok(super::home_dir()?.join(".config"))
}

/// Returns the user's data directory on Linux.
///
/// Respects `XDG_DATA_HOME` if set to an absolute path,
/// otherwise defaults to `~/.local/share/`.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn data_dir() -> Result<PathBuf> {
    if let Some(path) = xdg_path_if_valid("XDG_DATA_HOME") {
        return Ok(path);
    }
    Ok(super::home_dir()?.join(".local/share"))
}

/// Returns the user's cache directory on Linux.
///
/// Respects `XDG_CACHE_HOME` if set to an absolute path,
/// otherwise defaults to `~/.cache/`.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn cache_dir() -> Result<PathBuf> {
    if let Some(path) = xdg_path_if_valid("XDG_CACHE_HOME") {
        return Ok(path);
    }
    Ok(super::home_dir()?.join(".cache"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::test_utils::EnvGuard;

    #[test]
    fn config_dir_default_is_dot_config() {
        let mut env = EnvGuard::new();
        env.remove("XDG_CONFIG_HOME");

        let result = config_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(
            path.ends_with(".config"),
            "default config_dir should end with .config"
        );
    }

    #[test]
    fn config_dir_respects_xdg_config_home() {
        let mut env = EnvGuard::new();
        env.set("XDG_CONFIG_HOME", "/custom/config");

        let result = config_dir();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/custom/config"));
    }

    #[test]
    fn data_dir_default_is_local_share() {
        let mut env = EnvGuard::new();
        env.remove("XDG_DATA_HOME");

        let result = data_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(
            path.ends_with(".local/share"),
            "default data_dir should end with .local/share"
        );
    }

    #[test]
    fn data_dir_respects_xdg_data_home() {
        let mut env = EnvGuard::new();
        env.set("XDG_DATA_HOME", "/custom/data");

        let result = data_dir();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/custom/data"));
    }

    #[test]
    fn empty_xdg_vars_fall_back_to_default() {
        let mut env = EnvGuard::new();
        env.set("XDG_CONFIG_HOME", "");
        env.set("XDG_DATA_HOME", "");

        let config = config_dir().unwrap();
        let data = data_dir().unwrap();

        assert!(config.ends_with(".config"));
        assert!(data.ends_with(".local/share"));
    }

    #[test]
    fn relative_xdg_paths_fall_back_to_default() {
        let mut env = EnvGuard::new();
        env.set("XDG_CONFIG_HOME", "relative/path");
        env.set("XDG_DATA_HOME", "also/relative");

        let config = config_dir().unwrap();
        let data = data_dir().unwrap();

        assert!(
            config.ends_with(".config"),
            "relative XDG_CONFIG_HOME should fall back to ~/.config"
        );
        assert!(
            data.ends_with(".local/share"),
            "relative XDG_DATA_HOME should fall back to ~/.local/share"
        );
    }
}
