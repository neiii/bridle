//! Profile management commands.

use get_harness::{Harness, HarnessKind};

use crate::config::{BridleConfig, ProfileManager, ProfileName};
use crate::harness::HarnessConfig;

fn resolve_harness(name: &str) -> Option<Harness> {
    let kind = match name {
        "claude-code" | "claude" | "cc" => HarnessKind::ClaudeCode,
        "opencode" | "oc" => HarnessKind::OpenCode,
        "goose" => HarnessKind::Goose,
        _ => return None,
    };
    Some(Harness::new(kind))
}

fn get_manager() -> Option<ProfileManager> {
    let profiles_dir = BridleConfig::profiles_dir().ok()?;
    Some(ProfileManager::new(profiles_dir))
}

pub fn list_profiles(harness_name: &str) {
    let Some(harness) = resolve_harness(harness_name) else {
        eprintln!("Unknown harness: {harness_name}");
        eprintln!("Valid options: claude-code, opencode, goose");
        return;
    };

    let Some(manager) = get_manager() else {
        eprintln!("Could not find config directory");
        return;
    };

    match manager.list_profiles(&harness) {
        Ok(profiles) => {
            if profiles.is_empty() {
                println!("No profiles found for {}", harness.id());
            } else {
                println!("Profiles for {}:", harness.id());
                for profile in profiles {
                    println!("  {}", profile.as_str());
                }
            }
        }
        Err(e) => eprintln!("Error listing profiles: {e}"),
    }
}

pub fn show_profile(harness_name: &str, profile_name: &str) {
    let Some(harness) = resolve_harness(harness_name) else {
        eprintln!("Unknown harness: {harness_name}");
        return;
    };

    let Ok(name) = ProfileName::new(profile_name) else {
        eprintln!("Invalid profile name: {profile_name}");
        return;
    };

    let Some(manager) = get_manager() else {
        eprintln!("Could not find config directory");
        return;
    };

    match manager.show_profile(&harness, &name) {
        Ok(info) => {
            println!("Profile: {}", info.name);
            println!("Harness: {}", info.harness_id);
            println!("Active: {}", if info.is_active { "yes" } else { "no" });
            println!("Path: {}", info.path.display());
            if info.mcp_servers.is_empty() {
                println!("MCP Servers: (none)");
            } else {
                println!("MCP Servers:");
                for server in &info.mcp_servers {
                    println!("  - {}", server.name);
                }
            }
        }
        Err(e) => eprintln!("Error showing profile: {e}"),
    }
}

pub fn create_profile(harness_name: &str, profile_name: &str) {
    let Some(harness) = resolve_harness(harness_name) else {
        eprintln!("Unknown harness: {harness_name}");
        return;
    };

    let Ok(name) = ProfileName::new(profile_name) else {
        eprintln!("Invalid profile name: {profile_name}");
        return;
    };

    let Some(manager) = get_manager() else {
        eprintln!("Could not find config directory");
        return;
    };

    match manager.create_profile(&harness, &name) {
        Ok(path) => {
            println!("Created profile: {}", name.as_str());
            println!("Path: {}", path.display());
        }
        Err(e) => eprintln!("Error creating profile: {e}"),
    }
}

pub fn create_profile_from_current(harness_name: &str, profile_name: &str) {
    let Some(harness) = resolve_harness(harness_name) else {
        eprintln!("Unknown harness: {harness_name}");
        return;
    };

    let Ok(name) = ProfileName::new(profile_name) else {
        eprintln!("Invalid profile name: {profile_name}");
        return;
    };

    let Some(manager) = get_manager() else {
        eprintln!("Could not find config directory");
        return;
    };

    match manager.create_from_current(&harness, &name) {
        Ok(path) => {
            println!("Created profile from current config: {}", name.as_str());
            println!("Path: {}", path.display());
        }
        Err(e) => eprintln!("Error creating profile: {e}"),
    }
}

pub fn delete_profile(harness_name: &str, profile_name: &str) {
    let Some(harness) = resolve_harness(harness_name) else {
        eprintln!("Unknown harness: {harness_name}");
        return;
    };

    let Ok(name) = ProfileName::new(profile_name) else {
        eprintln!("Invalid profile name: {profile_name}");
        return;
    };

    let Some(manager) = get_manager() else {
        eprintln!("Could not find config directory");
        return;
    };

    match manager.delete_profile(&harness, &name) {
        Ok(()) => println!("Deleted profile: {}", name.as_str()),
        Err(e) => eprintln!("Error deleting profile: {e}"),
    }
}

pub fn edit_profile(harness_name: &str, profile_name: &str) {
    let Some(harness) = resolve_harness(harness_name) else {
        eprintln!("Unknown harness: {harness_name}");
        return;
    };

    let Ok(name) = ProfileName::new(profile_name) else {
        eprintln!("Invalid profile name: {profile_name}");
        return;
    };

    let Some(manager) = get_manager() else {
        eprintln!("Could not find config directory");
        return;
    };

    let profile_path = manager.profile_path(&harness, &name);
    if !profile_path.exists() {
        eprintln!("Profile not found: {profile_name}");
        return;
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new(&editor)
        .arg(&profile_path)
        .status();

    match status {
        Ok(s) if s.success() => println!("Edited profile: {profile_name}"),
        Ok(s) => eprintln!("Editor exited with status: {s}"),
        Err(e) => eprintln!("Failed to launch editor '{editor}': {e}"),
    }
}

pub fn diff_profiles(harness_name: &str, profile_name: &str, other_name: Option<&str>) {
    let Some(harness) = resolve_harness(harness_name) else {
        eprintln!("Unknown harness: {harness_name}");
        return;
    };

    let Ok(name) = ProfileName::new(profile_name) else {
        eprintln!("Invalid profile name: {profile_name}");
        return;
    };

    let Some(manager) = get_manager() else {
        eprintln!("Could not find config directory");
        return;
    };

    let profile_path = manager.profile_path(&harness, &name);
    if !profile_path.exists() {
        eprintln!("Profile not found: {profile_name}");
        return;
    }

    let other_path = if let Some(other) = other_name {
        let Ok(other_name) = ProfileName::new(other) else {
            eprintln!("Invalid profile name: {other}");
            return;
        };
        let path = manager.profile_path(&harness, &other_name);
        if !path.exists() {
            eprintln!("Profile not found: {other}");
            return;
        }
        path
    } else {
        match harness.config(&get_harness::Scope::Global) {
            Ok(path) => path,
            Err(_) => {
                eprintln!("Could not find current config for harness");
                return;
            }
        }
    };

    let status = std::process::Command::new("diff")
        .arg("-u")
        .arg(&profile_path)
        .arg(&other_path)
        .status();

    match status {
        Ok(s) if s.code() == Some(0) => println!("No differences"),
        Ok(s) if s.code() == Some(1) => {}
        Ok(s) => eprintln!("diff exited with status: {s}"),
        Err(e) => eprintln!("Failed to run diff: {e}"),
    }
}

pub fn switch_profile(harness_name: &str, profile_name: &str) {
    let Some(harness) = resolve_harness(harness_name) else {
        eprintln!("Unknown harness: {harness_name}");
        return;
    };

    let Ok(name) = ProfileName::new(profile_name) else {
        eprintln!("Invalid profile name: {profile_name}");
        return;
    };

    let Some(manager) = get_manager() else {
        eprintln!("Could not find config directory");
        return;
    };

    if !manager.profile_exists(&harness, &name) {
        eprintln!("Profile not found: {profile_name}");
        return;
    }

    let harness_id = harness.id();

    match manager.backup_current(&harness) {
        Ok(backup_path) => {
            println!("Backed up current config to: {}", backup_path.display());
        }
        Err(e) => {
            eprintln!("Warning: Could not backup current config: {e}");
        }
    }

    match manager.switch_profile(&harness, &name) {
        Ok(_) => {
            println!("Switched to profile: {}", name.as_str());
            println!("Harness: {harness_id}");
        }
        Err(e) => eprintln!("Error switching profile: {e}"),
    }
}
