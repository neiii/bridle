use harness_locate::{Harness, HarnessKind, InstallationStatus, Scope};
use serde::Serialize;

use crate::cli::output::{ResolvedFormat, output};
use crate::config::BridleConfig;

#[derive(Debug, Serialize)]
pub struct StatusOutput {
    pub harnesses: Vec<HarnessStatus>,
    pub active_profiles: Vec<ActiveProfile>,
}

#[derive(Debug, Serialize)]
pub struct HarnessStatus {
    pub id: String,
    pub name: String,
    pub status: String,
    pub config_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ActiveProfile {
    pub harness: String,
    pub profile: String,
}

pub fn display_status(format: ResolvedFormat) {
    let harnesses: Vec<HarnessStatus> = HarnessKind::ALL
        .iter()
        .map(|kind| {
            let harness = Harness::new(*kind);
            let status = match harness.installation_status() {
                Ok(InstallationStatus::FullyInstalled { .. }) => "installed",
                Ok(InstallationStatus::ConfigOnly { .. }) => "config only",
                Ok(InstallationStatus::BinaryOnly { .. }) => "binary only",
                _ => "not installed",
            };
            let config_path = if harness.is_installed() {
                harness
                    .config(&Scope::Global)
                    .ok()
                    .map(|p| p.display().to_string())
            } else {
                None
            };
            HarnessStatus {
                id: kind.to_string(),
                name: kind.to_string(),
                status: status.to_string(),
                config_path,
            }
        })
        .collect();

    let active_profiles: Vec<ActiveProfile> = BridleConfig::load()
        .map(|config| {
            config
                .active
                .iter()
                .map(|(harness, profile)| ActiveProfile {
                    harness: harness.clone(),
                    profile: profile.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    let status = StatusOutput {
        harnesses,
        active_profiles,
    };

    output(&status, format, |s| {
        println!("Harnesses:");
        for h in &s.harnesses {
            println!("  {} - {}", h.name, h.status);
            if let Some(path) = &h.config_path {
                println!("    Config: {}", path);
            }
        }

        if !s.active_profiles.is_empty() {
            println!("\nActive Profiles:");
            for ap in &s.active_profiles {
                println!("  {}: {}", ap.harness, ap.profile);
            }
        }
    });
}
