use std::path::PathBuf;

use blrs::{paths::FETCH_INTERVAL, search::VersionSearchQuery, BLRSConfig};
use chrono::Utc;
use clap::Subcommand;
use log::{debug, info};
use ls::LsFormat;
use serde::{Deserialize, Serialize};

use crate::{
    errs::{CommandError, IoErrorOrigin},
    repo_formatting::SortFormat,
    run,
    tasks::ConfigTask,
};

mod fetcher;
mod ls;
mod pull;
mod rm;
mod verify;

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    /// Fetches the latest builds from the Blender repositories. Does not download any build.
    Fetch {
        /// Ignore fetch timeouts.
        #[arg(short, long)]
        /// Runs fetching from repos in parallel using async features. Can trigger ratelimits if used recklessly.
        force: bool,
        #[arg(short, long)]
        parallel: bool,

        /// If true, if an error occurs then it will continue trying to fetch the rest of the repos.
        ///
        /// The return code of the program reflects the very first error that occurs.
        #[arg(short, long)]
        ignore_errors: bool,
    },

    /// Verifies that all the builds available to blrs has the required information. If one does not,
    /// we will run the build and gather data from it to generate the information we need
    Verify { repos: Option<Vec<String>> },

    /// Download a build from the saved database
    Pull {
        /// The version matchers to find the correct build.
        queries: Vec<String>,

        #[arg(short, long)]
        all_platforms: bool,
    },

    /// Tries to send a specified build to the trash.
    Rm {
        queries: Vec<String>,

        /// Tries to fully delete a file, and does not send the file to the trash
        #[arg(short, long)]
        no_trash: bool,
    },

    /// Lists builds available to download and builds that are installed
    Ls {
        #[arg(short, long)]
        format: Option<LsFormat>,

        #[arg(long)]
        sort_by: Option<SortFormat>,

        /// Filter out only builds that are installed.
        #[arg(short, long)]
        installed_only: bool,

        /// Show individual variants for remote builds.
        #[arg(short, long)]
        variants: bool,

        /// Shows all builds, even if they are not for your target os. Our filtering is not perfect. this may be necessary for you to find the proper build.
        #[arg(short, long)]
        all_builds: bool,
    },

    /// Launch a build
    Run {
        #[command(subcommand)]
        command: RunCommand,
    },
}

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum RunCommand {
    /// Open a specific file and assume the correct build
    File { path: PathBuf },

    /// Launch a specific build of blender
    Build {
        build: Option<String>,

        #[arg(raw(true))]
        args: Vec<String>,
    },
}

impl Command {
    pub fn eval(self, cfg: &BLRSConfig) -> Result<Vec<ConfigTask>, CommandError> {
        match self {
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
                let queries = strings_to_queries(queries)?;

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
            Command::Rm { queries, no_trash } => {
                let queries = strings_to_queries(queries)?;

                rm::remove_builds(cfg, queries, no_trash).map(|_| vec![])
            }
            Command::Ls {
                format,
                sort_by,
                installed_only,
                variants,
                all_builds,
            } => ls::list_builds(
                cfg,
                format.unwrap_or_default(),
                sort_by.unwrap_or_default(),
                installed_only,
                variants,
                all_builds,
            )
            .map(|()| vec![]),
            Command::Run { command } => {
                run::run(cfg, command, false).map(|_| vec![])
            }
        }
    }
}

fn strings_to_queries(queries: Vec<String>) -> Result<Vec<VersionSearchQuery>, CommandError> {
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

    Ok(queries)
}
