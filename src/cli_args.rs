use std::path::PathBuf;

use blrs::config::{BLRSConfig, FETCH_INTERVAL};
use clap::{arg, Parser};
use log::debug;
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::{commands::Commands, fetcher};

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
    /// Override the path to daily builds.
    #[arg(long)]
    pub daily_path: Option<PathBuf>,
    /// Override the path to experimental builds.
    #[arg(long)]
    pub experimental_path: Option<PathBuf>,
    /// Override the path to patch builds.
    #[arg(long)]
    pub patch_path: Option<PathBuf>,
}

impl Cli {
    pub fn apply_overrides(&self, config: &mut BLRSConfig) {
        if let Some(pth) = &self.library {
            config.paths.library = pth.clone()
        }
        if let Some(pth) = &self.daily_path {
            config.paths.daily = Some(pth.clone())
        }
        if let Some(pth) = &self.experimental_path {
            config.paths.experimental_path = Some(pth.clone())
        }
        if let Some(pth) = &self.patch_path {
            config.paths.patch_path = Some(pth.clone())
        }
    }

    /// Ok(bool) is whether the BLRSConfig should be saved
    pub fn eval(&self, cfg: &mut BLRSConfig) -> Result<bool, std::io::Error> {
        match self.commands.clone().unwrap() {
            Commands::Fetch { force, api_base } => {
                let api_base = match api_base {
                    Some(s) => match Url::parse(&s) {
                        Ok(u) => Some(u),
                        Err(_) => {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "Failed to parse api base argument into a URL",
                            ))
                        }
                    },
                    None => None,
                };

                let checked_time = cfg.last_time_checked.unwrap_or_default();
                let ready_time = checked_time + FETCH_INTERVAL;
                // Check if we are past the time we should be able to check for new builds.
                let ready_to_check = ready_time < chrono::Utc::now();
                if ready_to_check | force {
                    debug!["We are ready to check for new builds. Initializing tokio"];

                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(fetcher::fetch(cfg, api_base))
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, "Insufficient time has passed since the last fetch. It is unlikely that new builds will be available, and to conserve requests these will be skipped."))
                }
            }
            Commands::Pull { query } => todo!(),
            Commands::Ls { format, installed } => todo!(),
            Commands::Launch { query, commands } => todo!(),
            // Commands::ExportConfig { path } => todo!(),
        }
    }
}
