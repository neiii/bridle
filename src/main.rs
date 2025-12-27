mod cli;
mod config;
mod error;
mod harness;
mod tui;

use clap::Parser;
use cli::{Commands, ProfileCommands};

#[derive(Parser)]
#[command(name = "bridle")]
#[command(version, about = "Unified AI harness configuration manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Status => cli::status::display_status(),
        Commands::Init => cli::init::run_init(),
        Commands::Profile(profile_cmd) => match profile_cmd {
            ProfileCommands::List => cli::profile::list_profiles(),
            ProfileCommands::Show { name } => cli::profile::show_profile(&name),
            ProfileCommands::Apply { name } => cli::profile::apply_profile(&name),
            ProfileCommands::Add { name } => cli::profile::add_profile(&name),
            ProfileCommands::Remove { name } => cli::profile::remove_profile(&name),
        },
        Commands::Tui => cli::tui::run_tui(),
    }

    Ok(())
}
