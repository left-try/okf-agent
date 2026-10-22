//! Command-line parsing and dispatch for the provisional Rust interface.

use std::ffi::OsString;

use clap::{Parser, Subcommand};
use serde_json::json;

mod bundle;
mod docs;
mod index;
mod lifecycle;
mod run;
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
    /// Refresh repository-owned OKF knowledge.
    Update(lifecycle::UpdateArgs),
    /// Validate repository-owned OKF knowledge.
    Validate(lifecycle::ValidateArgs),
    /// Report repository and client readiness.
    Doctor(lifecycle::DoctorArgs),
    /// Install the Codex OKF skill.
    Install(lifecycle::InstallArgs),
    /// Record a repository decision.
    #[command(name = "record-decision")]
    RecordDecision(lifecycle::RecordDecisionArgs),
    /// Build, check, or serve the local documentation wiki.
    Docs(docs::DocsArgs),
    /// Prepare a repository for an agent and optionally launch its client.
    Run(run::RunArgs),
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
                    json!({ "implemented": ["capabilities", "index", "workflow show", "workflow explain", "workflow resolve", "bundle index", "bundle update", "bundle validate", "bundle search", "bundle backlinks", "bundle neighborhood", "bundle export", "update", "validate", "doctor", "install", "record-decision", "docs build", "docs check", "docs serve", "run"] })
                );
                0
            }
            Command::Workflow(args) => workflow::run(args),
            Command::Index(args) => index::run(args),
            Command::Bundle(args) => bundle::run(args),
            Command::Update(args) => lifecycle::update(args),
            Command::Validate(args) => lifecycle::validate(args),
            Command::Doctor(args) => lifecycle::doctor(args),
            Command::Install(args) => lifecycle::install(args),
            Command::RecordDecision(args) => lifecycle::record_decision(args),
            Command::Docs(args) => docs::run(args),
            Command::Run(args) => run::run(args),
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

fn root(path: &std::path::Path) -> std::path::PathBuf {
    okf_adapter_git::repo_root(path)
        .unwrap_or_else(|_| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}
