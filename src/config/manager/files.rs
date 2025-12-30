use std::path::Path;

use harness_locate::{Harness, Scope};

use crate::error::Result;
use crate::harness::HarnessConfig;

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

pub fn copy_resource_directories(
    harness: &Harness,
    to_profile: bool,
    profile_path: &Path,
) -> Result<()> {
    let config_dir = harness.config_dir()?;
    let mut copied_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();

    let resources = [
        harness.agents(&Scope::Global),
        harness.commands(&Scope::Global),
        harness.skills(&Scope::Global),
    ];

    for resource_result in resources {
        if let Ok(Some(dir)) = resource_result {
            let subdir_name = dir
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("resource");

            let profile_subdir = profile_path.join(subdir_name);
            let global_subdir = config_dir.join(subdir_name);

            let (src, dst) = if to_profile {
                (&global_subdir, &profile_subdir)
            } else {
                (&profile_subdir, &global_subdir)
            };

            if src.exists() && src.is_dir() {
                copy_dir_recursive(src, dst)?;
                copied_dirs.insert(subdir_name.to_string());
            }
        }
    }

    let fallback_dirs = [
        "agent", "agents", "command", "commands", "skill", "skills", "recipes",
    ];

    for subdir_name in fallback_dirs {
        if copied_dirs.contains(subdir_name) {
            continue;
        }

        let profile_subdir = profile_path.join(subdir_name);
        let global_subdir = config_dir.join(subdir_name);

        let (src, dst) = if to_profile {
            (&global_subdir, &profile_subdir)
        } else {
            (&profile_subdir, &global_subdir)
        };

        if src.exists() && src.is_dir() {
            copy_dir_recursive(src, dst)?;
        }
    }

    Ok(())
}
