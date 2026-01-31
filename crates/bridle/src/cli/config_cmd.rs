use harness_locate::{Harness, HarnessKind};

use crate::config::BridleConfig;
use crate::error::{Error, Result};
use crate::harness::HarnessConfig;

pub fn set_config(key: &str, value: &str) -> Result<()> {
    match key {
        "profile_marker" => set_profile_marker(value),
        _ => Err(Error::UnknownSetting(key.to_string())),
    }
}

pub fn get_config(key: &str) -> Result<()> {
    let config = BridleConfig::load()?;

    match key {
        "profile_marker" => println!("{}", config.profile_marker),
        _ => return Err(Error::UnknownSetting(key.to_string())),
    }
    Ok(())
}

fn set_profile_marker(value: &str) -> Result<()> {
    let enabled = match value.to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => return Err(Error::InvalidValue(value.to_string())),
    };

    let mut config = BridleConfig::load().unwrap_or_default();
    config.set_profile_marker(enabled);
    config.save()?;

    if !enabled {
        cleanup_all_marker_files();
    }

    println!("profile_marker = {}", enabled);
    Ok(())
}

fn cleanup_all_marker_files() {
    for kind in HarnessKind::ALL {
        let harness = Harness::new(*kind);
        let Ok(config_dir) = harness.config_dir() else {
            continue;
        };
        let Ok(entries) = std::fs::read_dir(&config_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else {
                continue;
            };
            if name.starts_with("BRIDLE_PROFILE_") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}
