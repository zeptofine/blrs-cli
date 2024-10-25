use std::path::PathBuf;

use blrs::{
    config::{BLRSConfig, FETCH_INTERVAL},
    fetching::authentication::GithubAuthentication,
    search::query::VersionSearchQuery,
};
use chrono::Utc;
use clap::{arg, Parser};
use log::{debug, info};
use serde::{Deserialize, Serialize};

use crate::{
    commands::{Command, RunCommand},
    errs::{CommandError, IoErrorOrigin},
    fetcher,
    ls::list_builds,
    pull, run,
    tasks::ConfigTask,
    verify,
};

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Alias of blrs-cli launch.
    pub build_or_file: Option<String>,

    #[command(subcommand)]
    pub commands: Option<Command>,

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

    pub fn eval(self, cfg: &BLRSConfig) -> Result<Vec<ConfigTask>, CommandError> {
        match self.commands.unwrap() {
            Command::Fetch {
                force,
                parallel,
                ignore_errors,
            } => {
                let checked_time = cfg.history.last_time_checked.unwrap_or_default();
                let ready_time = checked_time + FETCH_INTERVAL;
                // Check if we are past the time we should be able to check for new builds.
                let ready_to_check = ready_time < chrono::Utc::now();

                if ready_to_check | force {
                    debug!["We are ready to check for new builds. Initializing tokio"];

                    let rt = tokio::runtime::Runtime::new().unwrap();
                    let result = rt.block_on(fetcher::fetch(cfg, parallel, ignore_errors));

                    if result.is_ok() {
                        info![
                            "{}",
                            ansi_term::Color::Green
                                .bold()
                                .paint("Fetching builds finished successfully")
                        ];
                    }

                    result
                        .map(|v| vec![v])
                        .map_err(|e| CommandError::IoError(IoErrorOrigin::Fetching, e))
                } else {
                    let time_remaining = ready_time - Utc::now();
                    Err(CommandError::FetchingTooFast {
                        remaining: time_remaining.num_seconds(),
                    })
                }
            }
            Command::Verify { repos } => verify::verify(cfg, repos).map(|_| vec![]),
            Command::Pull {
                queries,
                all_platforms,
            } => {
                // parse the query into an actual query
                let queries: Vec<(String, Result<_, _>)> = queries
                    .into_iter()
                    .map(|s| {
                        let try_from = VersionSearchQuery::try_from(s.as_str());
                        (s, try_from)
                    })
                    .collect();

                // Any of the queries failed to parse
                if let Some((s, Err(e))) = queries.iter().find(|(_, v)| v.is_err()) {
                    return Err(CommandError::CouldNotParseQuery(s.clone(), e.clone()));
                }
                // The query list is empty
                if queries.is_empty() {
                    return Err(CommandError::MissingQuery);
                }

                let queries: Vec<VersionSearchQuery> = queries
                    .into_iter()
                    .map(|(_, o)| {
                        debug!["{:?}", o];
                        o.unwrap()
                    })
                    .collect();

                debug!["We are ready to download new builds. Initializing tokio"];

                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_time()
                    .enable_io()
                    .build()
                    .expect("failed to create runtime");

                let result = rt.block_on(pull::pull_builds(cfg, queries, all_platforms));

                match result {
                    Ok(_) => {
                        info![
                            "{}",
                            ansi_term::Color::Green
                                .bold()
                                .paint("Downloading builds finished successfully")
                        ];
                        Ok(vec![])
                    }
                    Err(e) => Err(e),
                }
            }
            Command::Rm {
                query,
                commands,
                no_trash,
            } => todo!(),
            Command::Ls {
                format,
                sort_by,
                installed_only,
                variants,
                all_builds,
            } => list_builds(
                cfg,
                format.unwrap_or_default(),
                sort_by.unwrap_or_default(),
                installed_only,
                variants,
                all_builds,
            )
            .map(|_| vec![]),
            Command::Run { query, mut command } => {
                if let Some(q) = query {
                    if let Ok(q) = VersionSearchQuery::try_from(q.as_str()) {
                        command = Some(RunCommand::Build {
                            build_or_file: Some(q.to_string()),
                            open_last: false,
                        });
                    } else {
                        command = Some(RunCommand::File {
                            path: PathBuf::from(q),
                        });
                    }
                }

                let command = match command {
                    Some(c) => c,
                    None => return Err(CommandError::NotEnoughInput),
                };

                run::run(cfg, command, false).map(|_| vec![])
            }
            Command::GithubAuth { user, token } => {
                let auth = GithubAuthentication { user, token };
                Ok(vec![ConfigTask::UpdateGHAuth(auth)])
            }
        }
    }
}
