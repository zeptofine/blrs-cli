use std::io::Write;

use ansi_term::Color;
use blrs::config::{BLRSConfig, PROJECT_DIRS};
use chrono::Utc;
use clap::{CommandFactory, Parser};

use cli_args::Cli;
use commands::Commands;
use log::debug;

mod cli_args;
mod commands;
mod fetcher;
mod ls;
mod pull;
mod repo_formatting;
mod tasks;
mod verify;

fn main() -> Result<(), std::io::Error> {
    #[cfg(target_os = "windows")]
    let _ = ansi_term::enable_ansi_support();

    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let mut cli = Cli::parse();

    let cfgfigment = BLRSConfig::default_figment(None);
    let mut cfg: BLRSConfig = cfgfigment.extract().unwrap();
    cli.apply_overrides(&mut cfg);

    debug!("{cli:?}");
    debug!("{cfg:?}");

    match (&cli.build_or_file, &cli.commands) {
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

    let r = cli.eval(&cfg);

    let tasks = match r {
        Ok(b) => b,
        Err(e) => {
            println![
                "\n{}\n    {}: {:?}\n    {}",
                Color::Red.bold().paint("COMMAND EXECUTION ERROR:"),
                Color::Blue.paint("Type"),
                e.kind(),
                e.get_ref().unwrap()
            ];
            println![];
            return Err(e);
        }
    };

    let tasks_exist = !tasks.is_empty();
    for task in tasks {
        match task {
            tasks::ConfigTask::UpdateGHAuth(github_authentication) => {
                cfg.gh_auth = Some(github_authentication);
            }
            tasks::ConfigTask::UpdateLastTimeChecked => {
                let dt = Utc::now();
                cfg.last_time_checked = Some(dt);
            }
        }
    }

    if tasks_exist {
        // Save the configuration to a file
        let config_file = PROJECT_DIRS.config_local_dir().join("config.toml");

        let mut file = std::fs::File::create(config_file)?;
        let data = match toml::to_string_pretty(&cfg) {
            Ok(d) => d,
            Err(e) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!["Failed to save config data: {:?}", e],
                ))
            }
        };
        file.write_all(data.as_bytes())?;
    }

    Ok(())
}
