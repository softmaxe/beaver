use std::process::ExitCode;

use clap::Parser;
use rename_subtitles::cli::{run, Cli};

fn main() -> ExitCode {
    run(Cli::parse())
}
