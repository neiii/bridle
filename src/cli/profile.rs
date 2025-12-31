use harness_locate::{Harness, HarnessKind};
use serde::Serialize;

use crate::cli::output::{ResolvedFormat, output, output_list};
use crate::config::{BridleConfig, ProfileManager, ProfileName};
use crate::display::{ProfileNode, SectionKind, nodes_to_text, profile_to_nodes};
use crate::error::{Error, Result};
use crate::harness::HarnessConfig;

#[derive(Serialize)]
struct ProfileListEntry {
    name: String,
    harness_id: String,
    is_active: bool,
}

fn resolve_harness(name: &str) -> Result<Harness> {
    let kind = match name {
        "claude-code" | "claude" | "cc" => HarnessKind::ClaudeCode,
        "opencode" | "oc" => HarnessKind::OpenCode,
        "goose" => HarnessKind::Goose,
        "amp-code" | "amp" | "ampcode" => HarnessKind::AmpCode,
        _ => return Err(Error::UnknownHarness(name.to_string())),
    };
    Ok(Harness::new(kind))
}

fn get_manager() -> Result<ProfileManager> {
    let profiles_dir = BridleConfig::profiles_dir()?;
    Ok(ProfileManager::new(profiles_dir))
}

pub fn list_profiles(harness_name: &str, format: ResolvedFormat) -> Result<()> {
    let harness = resolve_harness(harness_name)?;
    let manager = get_manager()?;

    let active_profile: Option<String> = BridleConfig::load()
        .ok()
        .and_then(|c| c.active_profile_for(harness.id()).map(|s| s.to_string()));

    let profiles = manager.list_profiles(&harness)?;
    let entries: Vec<ProfileListEntry> = profiles
        .iter()
        .map(|p| ProfileListEntry {
            name: p.to_string(),
            harness_id: harness.id().to_string(),
            is_active: active_profile
                .as_ref()
                .map(|a| a == &p.to_string())
                .unwrap_or(false),
        })
        .collect();

    output_list(&entries, format, |entries| {
        if entries.is_empty() {
            println!("No profiles found for {}", harness.id());
        } else {
            println!("Profiles for {}:", harness.id());
            for entry in entries {
                let active = if entry.is_active { " (active)" } else { "" };
                println!("  {}{}", entry.name, active);
            }
        }
    });
    Ok(())
}

pub fn show_profile(harness_name: &str, profile_name: &str, format: ResolvedFormat) -> Result<()> {
    let harness = resolve_harness(harness_name)?;
    let name = ProfileName::new(profile_name)
        .map_err(|_| Error::InvalidProfileName(profile_name.to_string()))?;
    let manager = get_manager()?;

    let info = manager.show_profile(&harness, &name)?;
    output(&info, format, |info| print_profile_text(info, &harness));
    Ok(())
}

fn print_profile_text(info: &crate::config::ProfileInfo, harness: &harness_locate::Harness) {
    let mut nodes = profile_to_nodes(info);

    if info.is_active {
        let marker_exists = harness
            .config_dir()
            .ok()
            .map(|dir| dir.join(format!("BRIDLE_PROFILE_{}", info.name)).exists())
            .unwrap_or(false);
        if marker_exists && let Some(header) = nodes.first_mut() {
            header.children.push(
                ProfileNode::new(SectionKind::Field, "Marker")
                    .with_text(format!("BRIDLE_PROFILE_{}", info.name)),
            );
        }
    }

    print!("{}", nodes_to_text(&nodes));
}

pub fn create_profile(harness_name: &str, profile_name: &str) -> Result<()> {
    let harness = resolve_harness(harness_name)?;
    let name = ProfileName::new(profile_name)
        .map_err(|_| Error::InvalidProfileName(profile_name.to_string()))?;
    let manager = get_manager()?;

    let path = manager.create_profile(&harness, &name)?;
    println!("Created profile: {}", name.as_str());
    println!("Path: {}", path.display());
    Ok(())
}

pub fn create_profile_from_current(harness_name: &str, profile_name: &str) -> Result<()> {
    let harness = resolve_harness(harness_name)?;
    let name = ProfileName::new(profile_name)
        .map_err(|_| Error::InvalidProfileName(profile_name.to_string()))?;
    let manager = get_manager()?;

    let path = manager.create_from_current_with_resources(&harness, Some(&harness), &name)?;
    println!("Created profile from current config: {}", name.as_str());
    println!("Path: {}", path.display());
    Ok(())
}

pub fn delete_profile(harness_name: &str, profile_name: &str) -> Result<()> {
    let harness = resolve_harness(harness_name)?;
    let name = ProfileName::new(profile_name)
        .map_err(|_| Error::InvalidProfileName(profile_name.to_string()))?;
    let manager = get_manager()?;

    manager.delete_profile(&harness, &name)?;
    println!("Deleted profile: {}", name.as_str());
    Ok(())
}

pub fn edit_profile(harness_name: &str, profile_name: &str) -> Result<()> {
    let harness = resolve_harness(harness_name)?;
    let name = ProfileName::new(profile_name)
        .map_err(|_| Error::InvalidProfileName(profile_name.to_string()))?;
    let manager = get_manager()?;

    let profile_path = manager.profile_path(&harness, &name);
    if !profile_path.exists() {
        return Err(Error::ProfileNotFound(profile_name.to_string()));
    }

    let config = crate::config::BridleConfig::load().unwrap_or_default();
    let editor = config.editor();
    let status = std::process::Command::new(&editor)
        .arg(&profile_path)
        .status()?;

    if status.success() {
        println!("Edited profile: {profile_name}");
        Ok(())
    } else {
        Err(Error::Command(format!(
            "Editor exited with status: {status}"
        )))
    }
}

pub fn diff_profiles(
    harness_name: &str,
    profile_name: &str,
    other_name: Option<&str>,
) -> Result<()> {
    let harness = resolve_harness(harness_name)?;
    let name = ProfileName::new(profile_name)
        .map_err(|_| Error::InvalidProfileName(profile_name.to_string()))?;
    let manager = get_manager()?;

    let profile_path = manager.profile_path(&harness, &name);
    if !profile_path.exists() {
        return Err(Error::ProfileNotFound(profile_name.to_string()));
    }

    let other_path = if let Some(other) = other_name {
        let other_name =
            ProfileName::new(other).map_err(|_| Error::InvalidProfileName(other.to_string()))?;
        let path = manager.profile_path(&harness, &other_name);
        if !path.exists() {
            return Err(Error::ProfileNotFound(other.to_string()));
        }
        path
    } else {
        harness.config(&harness_locate::Scope::Global)?
    };

    let status = std::process::Command::new("diff")
        .arg("-u")
        .arg(&profile_path)
        .arg(&other_path)
        .status()?;

    match status.code() {
        Some(0) => println!("No differences"),
        Some(1) => {}
        _ => return Err(Error::Command(format!("diff exited with status: {status}"))),
    }
    Ok(())
}

pub fn switch_profile(harness_name: &str, profile_name: &str) -> Result<()> {
    let harness = resolve_harness(harness_name)?;
    let name = ProfileName::new(profile_name)
        .map_err(|_| Error::InvalidProfileName(profile_name.to_string()))?;
    let manager = get_manager()?;

    if !manager.profile_exists(&harness, &name) {
        return Err(Error::ProfileNotFound(profile_name.to_string()));
    }

    let harness_id = harness.id();

    match manager.backup_current(&harness) {
        Ok(backup_path) => {
            println!("Backed up current config to: {}", backup_path.display());
        }
        Err(e) => {
            println!("Warning: Could not backup current config: {e}");
        }
    }

    manager.switch_profile_with_resources(&harness, Some(&harness), &name)?;
    println!("Switched to profile: {}", name.as_str());
    println!("Harness: {harness_id}");
    Ok(())
}
