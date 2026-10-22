use clap::Args;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct UpdateArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
    #[arg(long="changed-files",num_args=0..)]
    pub changed_files: Option<Vec<String>>,
}
#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
}
#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
}
#[derive(Debug, Args)]
pub struct InstallArgs {
    #[arg(long = "skills-dir")]
    pub skills_dir: Option<PathBuf>,
}
#[derive(Debug, Args)]
pub struct RecordDecisionArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
    #[arg(long)]
    pub title: String,
    #[arg(long)]
    pub rationale: String,
    #[arg(long = "source")]
    pub sources: Vec<String>,
}

pub fn update(args: UpdateArgs) -> i32 {
    let root = root(&args.path);
    match okf_application::lifecycle::update(
        &root,
        args.changed_files.as_deref(),
        &okf_adapter_fs::lifecycle::FsLifecycle,
        &okf_adapter_git::Git,
        &okf_adapter_fs::discovery::FsSourceCatalog,
        &okf_adapter_sqlite::repository::SqliteIndexStore,
    ) {
        Ok(result) => emit(&result, 0),
        Err(error) => diagnostic(error),
    }
}
pub fn validate(args: ValidateArgs) -> i32 {
    let root = root(&args.path);
    match okf_application::lifecycle::validate(
        &root,
        &okf_adapter_fs::lifecycle::FsLifecycle,
        &okf_adapter_fs::discovery::FsSourceCatalog,
        &okf_adapter_sqlite::repository::SqliteIndexStore,
    ) {
        Ok(result) => emit(&result, if result.ok { 0 } else { 1 }),
        Err(error) => diagnostic(error),
    }
}
pub fn doctor(args: DoctorArgs) -> i32 {
    let root = root(&args.path);
    let validation = match okf_application::lifecycle::validate(
        &root,
        &okf_adapter_fs::lifecycle::FsLifecycle,
        &okf_adapter_fs::discovery::FsSourceCatalog,
        &okf_adapter_sqlite::repository::SqliteIndexStore,
    ) {
        Ok(result) => result,
        Err(error) => return diagnostic(error),
    };
    let clients = serde_json::json!({"codex":executable("codex"),"claude":executable("claude"),"gemini":executable("gemini")});
    let payload = serde_json::json!({"root":root.to_string_lossy(),"git":okf_adapter_git::git_dir(&root).is_ok(),"client":clients,"validation":validation});
    emit(&payload, if validation.ok { 0 } else { 1 })
}
pub fn install(args: InstallArgs) -> i32 {
    match okf_application::lifecycle::install_codex_skill(
        args.skills_dir.as_deref(),
        &okf_adapter_fs::lifecycle::FsLifecycle,
    ) {
        Ok(path) => emit(
            &serde_json::json!({"skill":path,"next":"Restart Codex to discover the skill, then work normally in any repository."}),
            0,
        ),
        Err(error) => diagnostic(error),
    }
}
pub fn record_decision(args: RecordDecisionArgs) -> i32 {
    let root = root(&args.path);
    match okf_application::lifecycle::record_decision(
        &root,
        &args.title,
        &args.rationale,
        &args.sources,
        &okf_adapter_fs::lifecycle::FsLifecycle,
    ) {
        Ok(path) => emit(&serde_json::json!({"path":path}), 0),
        Err(error) => diagnostic(error),
    }
}

fn root(path: &Path) -> PathBuf {
    okf_adapter_git::repo_root(path)
        .unwrap_or_else(|_| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}
fn executable(name: &str) -> bool {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .any(|directory| {
            ["", ".exe", ".cmd", ".bat"]
                .iter()
                .any(|extension| directory.join(format!("{name}{extension}")).is_file())
        })
}
fn diagnostic(error: okf_domain::diagnostics::Diagnostic) -> i32 {
    emit(&serde_json::json!({"error":error}), 2)
}
fn emit(value: &impl Serialize, code: i32) -> i32 {
    match serde_json::to_string_pretty(value) {
        Ok(output) => {
            println!("{output}");
            code
        }
        Err(error) => {
            eprintln!("cannot serialize lifecycle result: {error}");
            2
        }
    }
}
