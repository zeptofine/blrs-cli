use blrs::{config::BLRSConfig, search::query::VersionSearchQuery};
use clap::{CommandFactory, Parser};

use cli_args::Cli;
use commands::Commands;
use log::{debug, info};

mod cli_args;
mod commands;

fn main() -> Result<(), std::io::Error> {
    env_logger::init();

    let mut cli = Cli::parse();

    debug!("{cli:?}");

    let cfgfigment = BLRSConfig::default_figment();
    let mut cfg: BLRSConfig = cfgfigment.extract().unwrap();

    cli.apply_overrides(&mut cfg);

    info!("{cfg:?}");

    match (&cli.name, &cli.commands) {
        (None, None) => {
            return Cli::command().print_help();
        }
        // TODO: If possible, implement this using the Clap derive system
        (Some(_), Some(_)) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Specifying a file and a subcommand at the same time is not supported",
            ));
        }
        (Some(query), None) => {
            cli.commands = Some(Commands::Launch {
                query: Some(query.to_string()),
                commands: None,
            });
        }
        (None, Some(_)) => {}
    }

    if let Some(Commands::Launch {
        query: Some(s),
        commands: None,
    }) = &cli.commands
    {
        println!["{:?}", VersionSearchQuery::from(s.clone())];
    }

    // cli.commands
    Ok(())
}
