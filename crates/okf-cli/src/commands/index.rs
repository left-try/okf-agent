use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

#[derive(Debug, Args)]
pub struct IndexArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
}

pub fn run(args: IndexArgs) -> i32 {
    if let Err(error) = okf_adapter_fs::templates::initialize(&args.path, &[]) {
        println!("{}", serde_json::json!({"error": error}));
        return 2;
    }
    let result = okf_application::repository_index::build(
        &args.path,
        &okf_adapter_fs::discovery::FsSourceCatalog,
        &okf_adapter_sqlite::repository::SqliteIndexStore,
    );
    match result {
        Ok(report) => print_json(&report, 0),
        Err(error) => print_json(&serde_json::json!({"error": error}), 2),
    }
}

fn print_json(value: &impl Serialize, code: i32) -> i32 {
    match serde_json::to_string_pretty(value) {
        Ok(output) => {
            println!("{output}");
            code
        }
        Err(error) => {
            eprintln!("cannot serialize index result: {error}");
            2
        }
    }
}
