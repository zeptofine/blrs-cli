use std::path::PathBuf;

use blrs::{
    config::{BLRSConfig, FETCH_INTERVAL},
    fetching::authentication::GithubAuthentication,
    search::query::VersionSearchQuery,
};
use clap::{arg, Parser};
use log::{debug, info};
use serde::{Deserialize, Serialize};

use crate::{commands::Commands, fetcher, ls::list_builds, tasks::ConfigTask, verify};

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Alias of blrs-cli launch <file>.
    pub build_or_file: Option<String>,

    #[command(subcommand)]
    pub commands: Option<Commands>,

    /// Override the path to the library.
    #[arg(short, long)]
    pub library: Option<PathBuf>,
}

impl Cli {
    pub fn apply_overrides(&self, config: &mut BLRSConfig) {
        if let Some(pth) = &self.library {
            config.paths.library = pth.clone()
        }
    }

    /// Ok(bool) is whether the BLRSConfig should be saved
    pub fn eval(&self, cfg: &BLRSConfig) -> Result<Vec<ConfigTask>, std::io::Error> {
        match self.commands.clone().unwrap() {
            Commands::Fetch {
                force,
                ignore_errors,
            } => {
                let checked_time = cfg.last_time_checked.unwrap_or_default();
                let ready_time = checked_time + FETCH_INTERVAL;
                // Check if we are past the time we should be able to check for new builds.
                let ready_to_check = ready_time < chrono::Utc::now();
                if ready_to_check | force {
                    debug!["We are ready to check for new builds. Initializing tokio"];

                    let rt = tokio::runtime::Runtime::new().unwrap();
                    let result = rt.block_on(fetcher::fetch(cfg, ignore_errors));

                    if result.is_ok() {
                        info![
                            "{}",
                            ansi_term::Color::Green
                                .bold()
                                .paint("Fetching builds finished successfully")
                        ];
                    }

                    result.map(|v| vec![v])
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, "Insufficient time has passed since the last fetch. It is unlikely that new builds will be available, and to conserve requests these will be skipped."))
                }
            }
            Commands::Verify { repos } => verify::verify(&cfg, repos),
            Commands::Pull { query } => {
                // parse the query into an actual query
                let query = match VersionSearchQuery::try_from(query) {
                    Ok(q) => q,
                    Err(e) => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Failed to parse the query",
                        ))
                    }
                };

                todo!()
            }
            Commands::Ls {
                format,
                sort_by,
                installed_only,
            } => list_builds(
                cfg,
                format.unwrap_or_default(),
                sort_by.unwrap_or_default(),
                installed_only,
            )
            .map(|_| vec![]),
            Commands::Launch { query, commands } => todo!(),
            Commands::GithubAuth { user, token } => {
                let auth = GithubAuthentication { user, token };
                Ok(vec![ConfigTask::UpdateGHAuth(auth)])
            }
        }
    }
}
