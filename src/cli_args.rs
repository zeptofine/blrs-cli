use std::path::PathBuf;

use blrs::config::BLRSConfig;

use clap::{arg, Parser};
use serde::{Deserialize, Serialize};

use crate::{commands::Command, errs::CommandError, tasks::ConfigTask};

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
        self.commands.unwrap().eval(cfg)
    }
}
