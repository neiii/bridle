mod cli;
mod config;
mod display;
mod error;
mod harness;
mod tui;

use clap::Parser;
use cli::output::OutputFormat;
use cli::{Commands, ConfigCommands, ProfileCommands};

#[derive(Parser)]
#[command(name = "bridle")]
#[command(version, about = "Unified AI harness configuration manager")]
struct Cli {
    #[arg(long, short = 'o', default_value = "auto", global = true)]
    output: OutputFormat,

    #[command(subcommand)]
    command: Option<Commands>,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();
    let format = cli.output.resolve();

    match cli.command {
        None | Some(Commands::Tui) => cli::tui::run_tui()?,
        Some(Commands::Status) => cli::status::display_status(format),
        Some(Commands::Init) => cli::init::run_init()?,
        Some(Commands::Profile(profile_cmd)) => match profile_cmd {
            ProfileCommands::List { harness } => cli::profile::list_profiles(&harness, format)?,
            ProfileCommands::Show { harness, name } => {
                cli::profile::show_profile(&harness, &name, format)?
            }
            ProfileCommands::Create {
                harness,
                name,
                from_current,
            } => {
                if from_current {
                    cli::profile::create_profile_from_current(&harness, &name)?
                } else {
                    cli::profile::create_profile(&harness, &name)?
                }
            }
            ProfileCommands::Delete { harness, name } => {
                cli::profile::delete_profile(&harness, &name)?
            }
            ProfileCommands::Switch { harness, name } => {
                cli::profile::switch_profile(&harness, &name)?
            }
            ProfileCommands::Edit { harness, name } => cli::profile::edit_profile(&harness, &name)?,
            ProfileCommands::Diff {
                harness,
                name,
                other,
            } => cli::profile::diff_profiles(&harness, &name, other.as_deref())?,
        },
        Some(Commands::Config(config_cmd)) => match config_cmd {
            ConfigCommands::Set { key, value } => cli::config_cmd::set_config(&key, &value)?,
            ConfigCommands::Get { key } => cli::config_cmd::get_config(&key)?,
        },
    }

    Ok(())
}
