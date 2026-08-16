use std::process::ExitCode;

use clap::Parser;
use submv::cli::{run, Cli};

fn main() -> ExitCode {
    run(Cli::parse())
}
