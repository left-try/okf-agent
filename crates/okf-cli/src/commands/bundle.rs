use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Args)]
pub struct BundleArgs {
    #[command(subcommand)]
    command: BundleCommand,
}

#[derive(Debug, Subcommand)]
enum BundleCommand {
    Index {
        #[arg(long, required = true)]
        root: PathBuf,
        #[arg(long = "index-root", required = true)]
        index_root: PathBuf,
        #[arg(long)]
        wikilinks: bool,
    },
    Update {
        #[arg(long, required = true)]
        root: PathBuf,
        #[arg(long = "index-root", required = true)]
        index_root: PathBuf,
        #[arg(long)]
        wikilinks: bool,
    },
    Validate {
        #[arg(long, required = true)]
        root: PathBuf,
        #[arg(long = "index-root", required = true)]
        index_root: PathBuf,
    },
    Search {
        query: String,
        #[arg(long, required = true)]
        root: PathBuf,
        #[arg(long = "index-root", required = true)]
        index_root: PathBuf,
        #[arg(long, default_value_t = 10)]
        limit: isize,
    },
    Backlinks {
        path: String,
        #[arg(long, required = true)]
        root: PathBuf,
        #[arg(long = "index-root", required = true)]
        index_root: PathBuf,
        #[arg(long, default_value_t = 1)]
        depth: isize,
    },
    Neighborhood {
        path: String,
        #[arg(long, required = true)]
        root: PathBuf,
        #[arg(long = "index-root", required = true)]
        index_root: PathBuf,
        #[arg(long, default_value_t = 1)]
        depth: isize,
    },
    Export {
        #[arg(long, required = true)]
        root: PathBuf,
        #[arg(long = "index-root", required = true)]
        index_root: PathBuf,
    },
}

pub fn run(args: BundleArgs) -> i32 {
    match args.command {
        BundleCommand::Index {
            root,
            index_root,
            wikilinks,
        }
        | BundleCommand::Update {
            root,
            index_root,
            wikilinks,
        } => match perform_index(&root, &index_root, wikilinks) {
            Ok(report) => print_json(&report, 0),
            Err(error) => error_json(error),
        },
        BundleCommand::Validate { root, index_root } => {
            match perform_index(&root, &index_root, false) {
                Ok(report) => {
                    let result = okf_application::bundle::validation_result(&report);
                    print_json(&result, if report.conformant { 0 } else { 1 })
                }
                Err(error) => error_json(error),
            }
        }
        BundleCommand::Search {
            query,
            index_root,
            limit,
            ..
        } => with_index(&index_root, || {
            okf_application::bundle::search(
                &index_root,
                &query,
                limit.max(1) as usize,
                &okf_adapter_sqlite::bundle::SqliteBundleStore,
            )
            .and_then(to_value)
        }),
        BundleCommand::Backlinks {
            path,
            index_root,
            depth,
            ..
        } => with_index(&index_root, || {
            okf_application::bundle::graph(
                &index_root,
                &path,
                depth.max(0) as usize,
                true,
                &okf_adapter_sqlite::bundle::SqliteBundleStore,
            )
        }),
        BundleCommand::Neighborhood {
            path,
            index_root,
            depth,
            ..
        } => with_index(&index_root, || {
            okf_application::bundle::graph(
                &index_root,
                &path,
                depth.max(0) as usize,
                false,
                &okf_adapter_sqlite::bundle::SqliteBundleStore,
            )
        }),
        BundleCommand::Export { index_root, .. } => with_index(&index_root, || {
            okf_application::bundle::export(
                &index_root,
                &okf_adapter_sqlite::bundle::SqliteBundleStore,
            )
        }),
    }
}

fn perform_index(
    root: &Path,
    index_root: &Path,
    wikilinks: bool,
) -> Result<okf_application::bundle::BundleIndexReport, okf_domain::diagnostics::Diagnostic> {
    let root = std::fs::canonicalize(root).map_err(|error| {
        okf_domain::diagnostics::Diagnostic::new(
            okf_domain::diagnostics::ErrorKind::InvalidPath,
            format!("cannot resolve bundle root: {error}"),
        )
    })?;
    std::fs::create_dir_all(index_root).map_err(|error| {
        okf_domain::diagnostics::Diagnostic::new(
            okf_domain::diagnostics::ErrorKind::Io,
            error.to_string(),
        )
    })?;
    let index_root = std::fs::canonicalize(absolute_path(index_root)).map_err(|error| {
        okf_domain::diagnostics::Diagnostic::new(
            okf_domain::diagnostics::ErrorKind::Io,
            error.to_string(),
        )
    })?;
    okf_application::bundle::index(
        &root,
        &index_root,
        wikilinks,
        &okf_adapter_fs::bundle::FsBundleSource,
        &okf_adapter_sqlite::bundle::SqliteBundleStore,
    )
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}
fn with_index(
    index_root: &Path,
    operation: impl FnOnce() -> Result<Value, okf_domain::diagnostics::Diagnostic>,
) -> i32 {
    if let Err(error) = okf_application::bundle::require_index(index_root) {
        return error_json(error);
    }
    match operation() {
        Ok(value) => print_json(&value, 0),
        Err(error) => error_json(error),
    }
}
fn to_value<T: Serialize>(value: T) -> Result<Value, okf_domain::diagnostics::Diagnostic> {
    serde_json::to_value(value).map_err(|error| {
        okf_domain::diagnostics::Diagnostic::new(
            okf_domain::diagnostics::ErrorKind::Internal,
            error.to_string(),
        )
    })
}
fn error_json(error: okf_domain::diagnostics::Diagnostic) -> i32 {
    print_json(&serde_json::json!({"error":error}), 2)
}
fn print_json(value: &impl Serialize, code: i32) -> i32 {
    match serde_json::to_string_pretty(value) {
        Ok(output) => {
            println!("{output}");
            code
        }
        Err(error) => {
            eprintln!("cannot serialize bundle result: {error}");
            2
        }
    }
}
