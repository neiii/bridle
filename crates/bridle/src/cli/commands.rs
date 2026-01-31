//! CLI subcommand definitions.

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Show status of all harnesses.
    Status,

    /// Initialize bridle configuration.
    Init,

    /// Manage profiles.
    #[command(subcommand)]
    Profile(ProfileCommands),

    /// Launch terminal UI.
    Tui,

    /// Manage bridle settings.
    #[command(subcommand)]
    Config(ConfigCommands),

    /// Install skills from a GitHub repository.
    Install {
        /// GitHub repository URL or owner/repo shorthand.
        source: String,
        /// Force overwrite existing skills.
        #[arg(long, short)]
        force: bool,
    },

    /// Uninstall components from a profile.
    Uninstall {
        /// Harness name (claude-code, opencode, goose, amp-code, crush, copilot-cli).
        harness: String,
        /// Profile name.
        profile: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Set a configuration value.
    Set {
        /// Setting name (e.g., profile_marker).
        key: String,
        /// Value to set (true/false for booleans).
        value: String,
    },

    /// Get a configuration value.
    Get {
        /// Setting name.
        key: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ProfileCommands {
    /// List profiles for a harness.
    List {
        /// Harness name (claude-code, opencode, goose, amp-code, crush, copilot-cli).
        harness: String,
    },

    /// Show details of a specific profile.
    Show {
        /// Harness name.
        harness: String,
        /// Profile name.
        name: String,
    },

    /// Create a new profile.
    Create {
        /// Harness name.
        harness: String,
        /// Profile name.
        name: String,
        /// Copy current harness config to the new profile.
        #[arg(long)]
        from_current: bool,
    },

    /// Delete a profile.
    Delete {
        /// Harness name.
        harness: String,
        /// Profile name.
        name: String,
    },

    /// Switch to a profile (set as active).
    Switch {
        /// Harness name.
        harness: String,
        /// Profile name.
        name: String,
    },

    /// Edit a profile with $EDITOR.
    Edit {
        /// Harness name.
        harness: String,
        /// Profile name.
        name: String,
    },

    /// Compare two profiles or profile vs current config.
    Diff {
        /// Harness name.
        harness: String,
        /// First profile name.
        name: String,
        /// Second profile name (optional, defaults to current config).
        other: Option<String>,
    },
}
