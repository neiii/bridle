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
}

#[derive(Subcommand, Debug)]
pub enum ProfileCommands {
    /// List available profiles.
    List,

    /// Show details of a specific profile.
    Show {
        /// Profile name to show.
        name: String,
    },

    /// Apply a profile (activate its configuration).
    Apply {
        /// Profile name to apply.
        name: String,
    },

    /// Add a new profile.
    Add {
        /// Profile name to create.
        name: String,
    },

    /// Remove a profile.
    Remove {
        /// Profile name to remove.
        name: String,
    },
}
