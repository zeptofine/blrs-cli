use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum Commands {
    /// Fetches the latest builds from the Blender repositories
    Fetch {
        /// Ignore fetch timeouts.
        #[arg(short, long)]
        force: bool,
    },
    /// lists all downloaded builds
    Ls {
        #[arg(short, long)]
        format: Option<LsFormat>,
    },
    Launch {
        /// The version match or blendfile to open.
        ///
        /// Whether you intend it to be a version match or blendfile will be decided by
        /// checking if it is parseable as a valid version search query.
        /// If it is not, it is assumed you meant it to be a file.
        /// There may be false positives.
        query: Option<String>,

        #[command(subcommand)]
        commands: Option<LaunchCommands>,
    },
}

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum LaunchCommands {
    /// Open a specific file and assume the correct build
    File { path: PathBuf },

    /// Launch a specific build of blender
    Build {
        query: String,

        #[arg(short, long)]
        open_last: bool,
    },
    /// Launches the last build and file launched by blrs
    Last,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum, Serialize, Deserialize)]
pub enum LsFormat {
    #[default]
    Newline,
    Json,
    Toml,
}
