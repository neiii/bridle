//! Configuration management for bridle.

#![allow(dead_code)]
#![allow(unused_imports)]

mod bridle;
pub mod jsonc;
mod manager;
mod profile_name;
mod types;

pub use bridle::{BridleConfig, TuiConfig, ViewPreference};
pub use manager::ProfileManager;
pub use profile_name::{InvalidProfileName, ProfileName};
pub use types::{McpServerInfo, ProfileInfo, ResourceSummary};
