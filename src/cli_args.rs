use std::path::PathBuf;

use blrs::config::BLRSConfig;

use clap::{arg, Parser};
use serde::{Deserialize, Serialize};

use crate::{commands::{Command, CompletionResult}, errs::CommandError};

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub commands: Command,

    /// Override the path to the library.
    #[arg(short, long)]
    pub library: Option<PathBuf>,
}

impl Cli {
    pub fn apply_overrides(&self, config: &mut BLRSConfig) {
        if let Some(pth) = &self.library {
            config.paths.library.clone_from(pth);
        }
    }

    pub fn eval(self, cfg: &BLRSConfig) -> Result<CompletionResult, CommandError> {
        self.commands.eval(cfg)
    }
}
