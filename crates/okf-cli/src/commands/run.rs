use clap::Args;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct RunArgs {
    pub agent: String,
    #[arg(default_value = ".")]
    pub path: PathBuf,
    #[arg(long)]
    pub no_launch: bool,
}
pub fn client_name(agent: &str) -> &str {
    match agent {
        "cursor" => "cursor",
        "windsurf" => "windsurf",
        "claude-code" => "claude",
        "gemini-cli" => "gemini",
        _ => agent,
    }
}
pub fn run(args: RunArgs) -> i32 {
    let root = super::root(&args.path);
    let mut payload = match okf_application::run_agent::prepare(
        &root,
        &args.agent,
        &okf_adapter_fs::lifecycle::FsLifecycle,
        &okf_adapter_git::Git,
        &okf_adapter_fs::discovery::FsSourceCatalog,
        &okf_adapter_sqlite::repository::SqliteIndexStore,
        &okf_adapter_fs::docs::FsDocs,
    ) {
        Ok(p) => p,
        Err(e) => return diagnostic(e),
    };
    if args.no_launch {
        return emit(&payload, 0);
    }
    let executable = client_name(&args.agent);
    if !super::process::contains(executable) {
        payload["error"] = serde_json::Value::String(format!(
            "Client `{executable}` is not on PATH. Use --no-launch to prepare the repository."
        ));
        return emit(&payload, 2);
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).unwrap_or_default()
    );
    #[cfg(feature = "watcher")]
    let _watcher = watcher::start(root.clone());
    let status = super::process::status(executable);
    match status {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("could not launch {executable}: {e}");
            2
        }
    }
}
fn emit(value: &impl Serialize, code: i32) -> i32 {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
    code
}
fn diagnostic(error: okf_domain::diagnostics::Diagnostic) -> i32 {
    emit(&serde_json::json!({"error":error}), 2)
}
#[cfg(feature = "watcher")]
#[path = "run_watcher.rs"]
mod watcher;

#[cfg(test)]
mod tests {
    #[test]
    fn aliases_match_existing_cli() {
        assert_eq!(super::client_name("cursor"), "cursor");
        assert_eq!(super::client_name("windsurf"), "windsurf");
        assert_eq!(super::client_name("claude-code"), "claude");
        assert_eq!(super::client_name("gemini-cli"), "gemini");
        assert_eq!(super::client_name("custom"), "custom");
    }
}
