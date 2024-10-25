use std::path::PathBuf;

use clap::Subcommand;
use ls::LsFormat;
use serde::{Deserialize, Serialize};

use crate::repo_formatting::SortFormat;

pub mod fetcher;
pub mod ls;
pub mod pull;
pub mod verify;

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
        /// The version match or blendfile to open.
        ///
        /// Whether you intend it to be a version match or blendfile will be decided by
        /// checking if it is parseable as a valid version search query.
        /// If it is not, it is assumed you meant it to be a file.
        /// There may be false positives in the matcher parser if you name
        /// your blendfiles weirdly.
        query: Option<String>,

        #[command(subcommand)]
        command: Option<RunCommand>,
    },

    /// Saves authentication data for github.
    ///
    /// This is useful for remote repositories based on github releases.
    ///
    /// WARNING! This is not encrypted and is readily available in your config location.
    GithubAuth { user: String, token: String },
}

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum RunCommand {
    /// Open a specific file and assume the correct build
    File { path: PathBuf },

    /// Launch a specific build of blender
    Build {
        build_or_file: Option<String>,

        #[arg(short, long)]
        open_last: bool,
    },
}
