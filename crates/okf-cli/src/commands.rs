//! Command-line parsing and dispatch for the provisional Rust interface.

use std::ffi::OsString;

use clap::{Parser, Subcommand};
use serde_json::json;

mod bundle;
mod index;
mod workflow;

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
    /// Inspect or resolve the repository workflow policy.
    Workflow(workflow::WorkflowArgs),
    /// Build the disposable local repository index.
    Index(index::IndexArgs),
    /// Index and query a standalone Markdown bundle.
    Bundle(bundle::BundleArgs),
    /// Preserve a useful structured response for commands not ported yet.
    #[command(external_subcommand)]
    Unsupported(Vec<OsString>),
}

impl Cli {
    pub fn run(self) -> i32 {
        match self.command {
            Command::Capabilities => {
                println!(
                    "{}",
                    json!({ "implemented": ["capabilities", "index", "workflow show", "workflow explain", "workflow resolve", "bundle index", "bundle update", "bundle validate", "bundle search", "bundle backlinks", "bundle neighborhood", "bundle export"] })
                );
                0
            }
            Command::Workflow(args) => workflow::run(args),
            Command::Index(args) => index::run(args),
            Command::Bundle(args) => bundle::run(args),
            Command::Unsupported(arguments) => {
                let command = arguments
                    .first()
                    .and_then(|value| value.to_str())
                    .unwrap_or("unknown");
                let message =
                    format!("Rust command {command:?} is not implemented yet; use `okf-agent`.");
                println!(
                    "{}",
                    json!({"error": {"code": "not_implemented", "message": message}})
                );
                2
            }
        }
    }
}
