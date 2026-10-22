use clap::{Args, Subcommand};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct DocsArgs {
    #[command(subcommand)]
    pub command: DocsCommand,
}
#[derive(Debug, Subcommand)]
pub enum DocsCommand {
    Build {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Check {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Serve {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
}
pub fn run(args: DocsArgs) -> i32 {
    match args.command {
        DocsCommand::Build { path, output } => {
            let root = super::root(&path);
            match okf_application::docs::build(
                &root,
                output.as_deref(),
                &okf_adapter_fs::discovery::FsSourceCatalog,
                &okf_adapter_sqlite::repository::SqliteIndexStore,
                &okf_adapter_fs::docs::FsDocs,
            ) {
                Ok(report) => emit(&report, 0),
                Err(error) => diagnostic(error),
            }
        }
        DocsCommand::Check { path, output } => {
            let root = super::root(&path);
            match okf_application::docs::check(
                &root,
                output.as_deref(),
                &okf_adapter_fs::docs::FsDocs,
            ) {
                Ok(report) => emit(&report, if report.ok { 0 } else { 1 }),
                Err(error) => diagnostic(error),
            }
        }
        DocsCommand::Serve { path, host, port } => {
            let root = super::root(&path);
            if let Err(error) = okf_application::docs::build(
                &root,
                None,
                &okf_adapter_fs::discovery::FsSourceCatalog,
                &okf_adapter_sqlite::repository::SqliteIndexStore,
                &okf_adapter_fs::docs::FsDocs,
            ) {
                return diagnostic(error);
            }
            match okf_adapter_fs::web::serve(&root, &host, port) {
                Ok(()) => 0,
                Err(error) => diagnostic(error),
            }
        }
    }
}
fn emit(value: &impl Serialize, code: i32) -> i32 {
    match serde_json::to_string_pretty(value) {
        Ok(s) => {
            println!("{s}");
            code
        }
        Err(e) => {
            eprintln!("cannot serialize docs response: {e}");
            2
        }
    }
}
fn diagnostic(error: okf_domain::diagnostics::Diagnostic) -> i32 {
    emit(&serde_json::json!({"error":error}), 2)
}
