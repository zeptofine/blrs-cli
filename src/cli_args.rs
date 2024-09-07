use clap::{arg, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE")]
    name: Option<String>,

    #[command(subcommand)]
    commands: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Fetches the latest builds from the Blender repositories
    Fetch {
        /// Ignore fetch timeouts.
        #[arg(short, long)]
        force: bool,
    },
    /// lists all downloaded builds
    Ls {
        #[arg(short, long)]
        format: Option<ListFormat>,
    },
}

#[derive(Clone, Copy, Default, ValueEnum)]
enum ListFormat {
    #[default]
    Newline,
    Json,
    Toml,
}
