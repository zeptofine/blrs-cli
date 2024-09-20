use std::path::PathBuf;

use clap::Subcommand;
use serde::{Deserialize, Serialize};

use crate::ls::{LsFormat, SortFormat};

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum Commands {
    /// Fetches the latest builds from the Blender repositories. Does not download any build.
    Fetch {
        /// Ignore fetch timeouts.
        #[arg(short, long)]
        force: bool,

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
        /// The version matcher to find the correct build.
        query: String,
    },

    /// Lists builds available to download and builds that are installed
    Ls {
        #[arg(short, long)]
        format: Option<LsFormat>,

        #[arg(short, long)]
        sort_by: Option<SortFormat>,

        /// Filter out only builds that are installed.
        #[arg(short, long)]
        installed_only: bool,
    },
    /// Launch a build
    Launch {
        /// The version match or blendfile to open.
        ///
        /// Whether you intend it to be a version match or blendfile will be decided by
        /// checking if it is parseable as a valid version search query.
        /// If it is not, it is assumed you meant it to be a file.
        /// There may be false positives in the matcher parser.
        query: Option<String>,

        #[command(subcommand)]
        commands: Option<LaunchCommands>,
    },

    /// Saves authentication data for github.
    ///
    /// This is useful for remote repositories based on github releases.
    ///
    /// WARNING! This is not encrypted and is readily available in your config location.
    GithubAuth { user: String, token: String },
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
