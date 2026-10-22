//! Command-line parsing and dispatch for the provisional Rust interface.

use std::ffi::OsString;

use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(
    name = "okf-agent-rs",
    version,
    about = "Rust implementation of OKF Agent"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show the commands implemented by this Rust binary.
    Capabilities,
    /// Preserve a useful structured response for commands not ported yet.
    #[command(external_subcommand)]
    Unsupported(Vec<OsString>),
}

impl Cli {
    pub fn run(self) -> i32 {
        match self.command {
            Command::Capabilities => {
                println!("{}", json!({ "implemented": ["capabilities"] }));
                0
            }
            Command::Unsupported(arguments) => {
                let command = arguments
                    .first()
                    .and_then(|value| value.to_str())
                    .unwrap_or("unknown");
                let message =
                    format!("Rust command {command:?} is not implemented yet; use `okf-agent`.");
                println!(
                    "{}",
                    json!({
                        "error": {
                            "code": "not_implemented",
                            "message": message,
                        }
                    })
                );
                2
            }
        }
    }
}
