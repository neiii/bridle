use std::path::Path;

use harness_locate::{Harness, Scope};

use crate::error::Result;
use crate::harness::HarnessConfig;

/// Directories to skip when copying profiles
const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".DS_Store",
    "Thumbs.db",
    "__pycache__",
    "node_modules",
];

pub fn copy_config_files(
    harness: &dyn HarnessConfig,
    source_is_live: bool,
    profile_path: &Path,
) -> Result<()> {
    use std::collections::HashSet;

    let config_dir = harness.config_dir()?;
    let mut copied_files: HashSet<std::path::PathBuf> = HashSet::new();

    if source_is_live {
        if config_dir.exists() {
            for entry in std::fs::read_dir(&config_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    let dest = profile_path.join(entry.file_name());
                    std::fs::copy(entry.path(), &dest)?;
                    if let Ok(canonical) = entry.path().canonicalize() {
                        copied_files.insert(canonical);
                    }
                }
            }
        }

        if let Some(mcp_path) = harness.mcp_config_path() {
            let dominated = mcp_path
                .canonicalize()
                .map(|c| copied_files.contains(&c))
                .unwrap_or(false);

            if !dominated
                && mcp_path.exists()
                && mcp_path.is_file()
                && let Some(filename) = mcp_path.file_name()
            {
                let dest = profile_path.join(filename);
                std::fs::copy(&mcp_path, dest)?;
            }
        }
    } else {
        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir)?;
        }

        let mcp_filename = harness
            .mcp_config_path()
            .and_then(|p| p.file_name().map(|f| f.to_os_string()));

        for entry in std::fs::read_dir(profile_path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let filename = entry.file_name();

                if let Some(ref mcp_name) = mcp_filename
                    && &filename == mcp_name
                    && let Some(mcp_path) = harness.mcp_config_path()
                {
                    std::fs::copy(entry.path(), &mcp_path)?;
                    continue;
                }

                let dest = config_dir.join(&filename);
                std::fs::copy(entry.path(), dest)?;
            }
        }
    }

    Ok(())
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Copy directory recursively, preserving symlinks and skipping excluded dirs.
/// Continues on errors (logs warning) rather than aborting.
pub fn copy_dir_filtered(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Warning: Failed to read entry in {}: {}", src.display(), e);
                continue;
            }
        };

        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();

        if EXCLUDED_DIRS.iter().any(|&ex| name_str == ex) {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(&file_name);
        let file_type = entry.file_type()?;

        #[cfg(unix)]
        if file_type.is_symlink() {
            if let Ok(target) = std::fs::read_link(&src_path) {
                let _ = std::fs::remove_file(&dst_path);
                if let Err(e) = std::os::unix::fs::symlink(&target, &dst_path) {
                    eprintln!(
                        "Warning: Failed to create symlink {}: {}",
                        dst_path.display(),
                        e
                    );
                }
            }
            continue;
        }

        if file_type.is_dir() {
            if let Err(e) = copy_dir_filtered(&src_path, &dst_path) {
                eprintln!(
                    "Warning: Failed to copy directory {}: {}",
                    src_path.display(),
                    e
                );
            }
        } else if let Err(e) = std::fs::copy(&src_path, &dst_path) {
            eprintln!("Warning: Failed to copy file {}: {}", src_path.display(), e);
        }
    }

    Ok(())
}

pub fn copy_resource_directories(
    harness: &Harness,
    to_profile: bool,
    profile_path: &Path,
) -> Result<()> {
    let config_dir = harness.config_dir()?;

    let source_dir = if to_profile {
        &config_dir
    } else {
        profile_path
    };

    if !source_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(source_dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if !file_type.is_dir() {
            continue;
        }

        let dir_name = entry.file_name();
        let name_str = dir_name.to_string_lossy();

        if EXCLUDED_DIRS.iter().any(|&ex| name_str == ex) {
            continue;
        }

        let src = entry.path();
        let dst = if to_profile {
            profile_path.join(&dir_name)
        } else {
            config_dir.join(&dir_name)
        };

        copy_dir_recursive(&src, &dst)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn copy_dir_filtered_skips_excluded_directories() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();

        fs::create_dir(src.path().join(".git")).unwrap();
        fs::write(src.path().join(".git/config"), "git config").unwrap();
        fs::create_dir(src.path().join("plugins")).unwrap();
        fs::write(src.path().join("plugins/myplugin.json"), "{}").unwrap();
        fs::write(src.path().join("config.json"), "{}").unwrap();

        copy_dir_filtered(src.path(), dst.path()).unwrap();

        assert!(!dst.path().join(".git").exists());
        assert!(dst.path().join("plugins").exists());
        assert!(dst.path().join("plugins/myplugin.json").exists());
        assert!(dst.path().join("config.json").exists());
    }

    #[test]
    fn copy_dir_filtered_copies_nested_directories() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();

        fs::create_dir_all(src.path().join("hooks/pre-commit")).unwrap();
        fs::write(src.path().join("hooks/pre-commit/run.sh"), "#!/bin/bash").unwrap();

        copy_dir_filtered(src.path(), dst.path()).unwrap();

        assert!(dst.path().join("hooks/pre-commit/run.sh").exists());
        let content = fs::read_to_string(dst.path().join("hooks/pre-commit/run.sh")).unwrap();
        assert_eq!(content, "#!/bin/bash");
    }

    #[cfg(unix)]
    #[test]
    fn copy_dir_filtered_preserves_symlinks() {
        use std::os::unix::fs::symlink;

        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();

        fs::write(src.path().join("target.txt"), "target content").unwrap();
        symlink("target.txt", src.path().join("link.txt")).unwrap();

        copy_dir_filtered(src.path(), dst.path()).unwrap();

        let link_path = dst.path().join("link.txt");
        assert!(
            link_path
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let link_target = fs::read_link(&link_path).unwrap();
        assert_eq!(link_target.to_str().unwrap(), "target.txt");
    }
}
