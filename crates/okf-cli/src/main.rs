mod commands;

use clap::Parser;
use commands::Cli;
use std::process;

fn main() {
    let exit_code = Cli::parse().run();
    if exit_code != 0 {
        process::exit(exit_code);
    }
}
