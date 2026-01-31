//! Installation management for bridle.

#![allow(dead_code)]
#![allow(unused_imports)]

pub mod discovery;
pub mod installer;
pub mod manifest;
pub mod mcp_config;
pub mod mcp_installer;
pub mod types;
pub mod uninstaller;

pub use discovery::{DiscoveryError, discover_skills};
pub use types::*;
