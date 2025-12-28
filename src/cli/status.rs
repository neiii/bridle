//! Status display functionality.

use harness_locate::{Harness, HarnessKind, InstallationStatus, Scope};

use crate::config::BridleConfig;

pub fn display_status() {
    println!("Harnesses:");
    for kind in HarnessKind::ALL {
        let harness = Harness::new(*kind);
        let status = match harness.installation_status() {
            Ok(InstallationStatus::FullyInstalled { .. }) => "installed",
            Ok(InstallationStatus::ConfigOnly { .. }) => "config only",
            Ok(InstallationStatus::BinaryOnly { .. }) => "binary only",
            _ => "not installed",
        };
        println!("  {} - {}", kind, status);

        if harness.is_installed()
            && let Ok(config) = harness.config(&Scope::Global)
        {
            println!("    Config: {}", config.display());
        }
    }

    match BridleConfig::load() {
        Ok(config) if !config.active.is_empty() => {
            println!("\nActive Profiles:");
            for (harness, profile) in &config.active {
                println!("  {}: {}", harness, profile);
            }
        }
        _ => {}
    }
}
