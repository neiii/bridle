//! Init command implementation.

use get_harness::{Harness, HarnessKind};

use crate::config::{BridleConfig, ProfileManager};

pub fn run_init() {
    let config_dir = match BridleConfig::config_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };

    let config_path = match BridleConfig::config_path() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };

    if config_path.exists() {
        println!("Already initialized at {}", config_dir.display());
        return;
    }

    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        eprintln!("Failed to create config directory: {e}");
        return;
    }

    let profiles_dir = config_dir.join("profiles");
    if let Err(e) = std::fs::create_dir_all(&profiles_dir) {
        eprintln!("Failed to create profiles directory: {e}");
        return;
    }

    let config = BridleConfig::default();
    if let Err(e) = config.save() {
        eprintln!("Failed to write config: {e}");
        return;
    }

    let manager = ProfileManager::new(profiles_dir);
    for kind in HarnessKind::ALL {
        let harness = Harness::new(*kind);
        let _ = manager.create_from_current_if_missing(&harness);
    }

    println!("Initialized bridle at {}", config_dir.display());
}
