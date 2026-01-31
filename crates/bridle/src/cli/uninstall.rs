//! CLI uninstall command implementation.

use std::io::IsTerminal;
use std::path::Path;

use color_eyre::eyre::{Result, eyre};
use dialoguer_multiselect::MultiSelect;
use dialoguer_multiselect::theme::ColorfulTheme;

use crate::cli::profile::resolve_harness;
use crate::config::BridleConfig;
use crate::harness::HarnessConfig;
use crate::install::uninstaller::uninstall_components;
use crate::install::{ComponentType, InstallTarget};

pub fn run(harness: &str, profile: &str) -> Result<()> {
    if !std::io::stdin().is_terminal() {
        return Err(eyre!("Interactive mode requires a terminal."));
    }

    let harness_obj = resolve_harness(harness)?;
    let harness_id = harness_obj.id();

    let profiles_dir = BridleConfig::profiles_dir()?;
    let profile_name = crate::config::ProfileName::new(profile)?;

    let profile_path = profiles_dir.join(harness_id).join(profile);
    if !profile_path.exists() {
        return Err(eyre!("Profile not found: {}/{}", harness_id, profile));
    }

    let components = list_installed_components(&profile_path)?;

    if components.is_empty() {
        eprintln!("No components installed in {}/{}", harness_id, profile);
        return Ok(());
    }

    let component_labels: Vec<String> = components
        .iter()
        .map(|(name, comp_type)| format!("{:?}: {}", comp_type, name))
        .collect();

    let Some(selected_indices) = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select components to uninstall (Esc to cancel)")
        .items(&component_labels)
        .interact_opt()?
    else {
        eprintln!("Cancelled");
        return Ok(());
    };

    if selected_indices.is_empty() {
        eprintln!("No components selected");
        return Ok(());
    }

    let selected_components: Vec<_> = selected_indices
        .iter()
        .map(|&i| components[i].clone())
        .collect();

    let target = InstallTarget {
        harness: harness_id.to_string(),
        profile: profile_name,
    };

    eprintln!("\nUninstalling from {}/{}...", harness_id, profile);

    let report = uninstall_components(&selected_components, &target);

    for success in &report.removed {
        eprintln!(
            "  - Removed: {} ({})",
            success.component, success.component_type
        );
    }

    for error in &report.errors {
        eprintln!(
            "  ! Error removing {} ({}): {}",
            error.component, error.component_type, error.error
        );
    }

    eprintln!("\nDone!");
    Ok(())
}

fn list_installed_components(profile_path: &Path) -> Result<Vec<(String, ComponentType)>> {
    let mut components = Vec::new();

    let component_types = [
        (ComponentType::Skill, "skills"),
        (ComponentType::Agent, "agents"),
        (ComponentType::Command, "commands"),
    ];

    for (comp_type, dir_name) in component_types {
        let dir = profile_path.join(dir_name);
        if !dir.exists() {
            continue;
        }

        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && let Some(name) = entry.file_name().to_str()
            {
                components.push((name.to_string(), comp_type));
            }
        }
    }

    Ok(components)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolve_harness_canonicalizes_copilot_alias() {
        let harness = resolve_harness("copilot").unwrap();
        assert_eq!(harness.id(), "copilot-cli");
    }

    #[test]
    fn resolve_harness_accepts_canonical_copilot() {
        let harness = resolve_harness("copilot-cli").unwrap();
        assert_eq!(harness.id(), "copilot-cli");
    }

    #[test]
    fn profile_path_uses_canonical_harness_id() {
        let temp = TempDir::new().unwrap();
        let profiles_dir = temp.path();

        let canonical_dir = profiles_dir.join("copilot-cli").join("test");
        std::fs::create_dir_all(&canonical_dir).unwrap();

        let harness = resolve_harness("copilot").unwrap();
        let harness_id = harness.id();

        let profile_path = profiles_dir.join(harness_id).join("test");

        assert_eq!(profile_path, canonical_dir);
        assert!(profile_path.exists());
    }
}
