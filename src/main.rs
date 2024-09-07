use clap::{CommandFactory, Parser};
use cli_args::Cli;

mod cli_args;

fn main() {
    let cli = Cli::parse();
}
