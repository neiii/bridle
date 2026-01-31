//! CLI module for bridle.

mod commands;
pub mod config_cmd;
pub mod init;
pub mod install;
pub mod output;
pub mod profile;
pub mod status;
pub mod tui;
pub mod uninstall;

pub use commands::{Commands, ConfigCommands, ProfileCommands};
