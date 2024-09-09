use std::path::PathBuf;

use blrs::config::BLRSConfig;
use clap::{arg, Parser};
use serde::{Deserialize, Serialize};

use crate::commands::Commands;

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Alias of blrs-cli launch <file>.
    pub name: Option<String>,

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
}
