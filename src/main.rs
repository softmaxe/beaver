use std::process::ExitCode;

use beaver::cli::{run, Cli};
use clap::Parser;

fn main() -> ExitCode {
    run(Cli::parse())
}
