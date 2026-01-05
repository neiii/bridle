use std::path::Path;

use harness_locate::{Harness, HarnessKind, Scope};

use crate::error::Result;
use crate::harness::HarnessConfig;
use crate::install::installer::{sanitize_name_for_opencode, transform_skill_for_opencode};

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
                let file_type = entry.file_type()?;
                let dest = profile_path.join(entry.file_name());

                if file_type.is_file() {
                    std::fs::copy(entry.path(), &dest)?;
                    if let Ok(canonical) = entry.path().canonicalize() {
                        copied_files.insert(canonical);
                    }
                } else if file_type.is_dir() {
                    copy_dir_filtered(&entry.path(), &dest)?;
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

pub fn copy_all_contents(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_filtered(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
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

/// Canonical directory names used inside profiles for resource storage.
/// These are bridle's internal convention - harness-locate maps them to actual paths.
pub const CANONICAL_COMMANDS_DIR: &str = "commands";
pub const CANONICAL_AGENTS_DIR: &str = "agents";
pub const CANONICAL_SKILLS_DIR: &str = "skills";
pub const CANONICAL_PLUGINS_DIR: &str = "plugins";

fn copy_skills_for_opencode(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }

    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();

        if !src_path.is_dir() {
            continue;
        }

        let original_name = entry.file_name().to_string_lossy().to_string();
        let sanitized_name = sanitize_name_for_opencode(&original_name);
        let dst_skill_dir = dst.join(&sanitized_name);

        std::fs::create_dir_all(&dst_skill_dir)?;

        for skill_entry in std::fs::read_dir(&src_path)? {
            let skill_entry = skill_entry?;
            let skill_src = skill_entry.path();
            let skill_dst = dst_skill_dir.join(skill_entry.file_name());

            if skill_src.is_file() {
                let is_skill_md = skill_entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("SKILL.md");

                if is_skill_md {
                    let content = std::fs::read_to_string(&skill_src)?;
                    let transformed = transform_skill_for_opencode(&content, &sanitized_name);
                    std::fs::write(&skill_dst, transformed)?;
                } else {
                    std::fs::copy(&skill_src, &skill_dst)?;
                }
            } else if skill_src.is_dir() {
                copy_dir_filtered(&skill_src, &skill_dst)?;
            }
        }
    }

    Ok(())
}

/// Copy resource directories between profile and harness using harness-aware paths.
///
/// When `to_profile` is true: harness paths → canonical profile dirs
/// When `to_profile` is false: canonical profile dirs → harness paths
///
/// Uses canonical names inside profiles for cross-harness portability.
pub fn copy_resource_directories(
    harness: &Harness,
    to_profile: bool,
    profile_path: &Path,
) -> Result<()> {
    let scope = Scope::Global;

    let resources: Vec<(&str, Option<std::path::PathBuf>)> = vec![
        (
            CANONICAL_COMMANDS_DIR,
            harness.commands(&scope).ok().flatten().map(|r| r.path),
        ),
        (
            CANONICAL_AGENTS_DIR,
            harness.agents(&scope).ok().flatten().map(|r| r.path),
        ),
        (
            CANONICAL_SKILLS_DIR,
            harness.skills(&scope).ok().flatten().map(|r| r.path),
        ),
        (
            CANONICAL_PLUGINS_DIR,
            harness.plugins(&scope).ok().flatten().map(|r| r.path),
        ),
    ];

    for (canonical_name, harness_path) in resources {
        let Some(harness_path) = harness_path else {
            continue;
        };

        let profile_resource = profile_path.join(canonical_name);

        let (src, dst) = if to_profile {
            (harness_path.as_path(), profile_resource.as_path())
        } else {
            (profile_resource.as_path(), harness_path.as_path())
        };

        if src.exists() && src.is_dir() {
            let is_skills_to_opencode = !to_profile
                && canonical_name == CANONICAL_SKILLS_DIR
                && matches!(harness.kind(), HarnessKind::OpenCode);

            if is_skills_to_opencode {
                copy_skills_for_opencode(src, dst)?;
            } else {
                copy_dir_filtered(src, dst)?;
            }
        }
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

    #[test]
    fn copy_config_files_copies_directories_when_saving() {
        use crate::harness::HarnessConfig;
        use std::path::PathBuf;

        struct TestHarness(PathBuf);
        impl HarnessConfig for TestHarness {
            fn id(&self) -> &str {
                "test"
            }
            fn config_dir(&self) -> crate::error::Result<PathBuf> {
                Ok(self.0.clone())
            }
            fn installation_status(
                &self,
            ) -> crate::error::Result<harness_locate::InstallationStatus> {
                Ok(harness_locate::InstallationStatus::NotInstalled)
            }
            fn mcp_filename(&self) -> Option<String> {
                None
            }
            fn mcp_config_path(&self) -> Option<PathBuf> {
                None
            }
            fn parse_mcp_servers(
                &self,
                _: &str,
                _: &str,
            ) -> crate::error::Result<Vec<(String, bool)>> {
                Ok(vec![])
            }
        }

        let temp = TempDir::new().unwrap();
        let config_dir = temp.path().join("config");
        let profile_dir = temp.path().join("profile");
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&profile_dir).unwrap();

        fs::write(config_dir.join("settings.json"), "{}").unwrap();
        fs::create_dir_all(config_dir.join("custom-dir/nested")).unwrap();
        fs::write(config_dir.join("custom-dir/data.txt"), "precious").unwrap();
        fs::write(config_dir.join("custom-dir/nested/deep.txt"), "deep data").unwrap();

        let harness = TestHarness(config_dir);
        copy_config_files(&harness, true, &profile_dir).unwrap();

        assert!(profile_dir.join("settings.json").exists());
        assert!(profile_dir.join("custom-dir").exists());
        assert!(profile_dir.join("custom-dir/data.txt").exists());
        assert_eq!(
            fs::read_to_string(profile_dir.join("custom-dir/data.txt")).unwrap(),
            "precious"
        );
        assert!(profile_dir.join("custom-dir/nested/deep.txt").exists());
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
