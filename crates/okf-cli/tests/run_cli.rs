use serde_json::Value;
use std::{fs, process::Command};

fn temp() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let p = std::env::temp_dir().join(format!(
        "okf-run-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn run_no_launch_prepares_repository_and_returns_context() {
    let root = temp();
    let output = Command::new(env!("CARGO_BIN_EXE_okf-agent-rs"))
        .args(["run", "codex", root.to_str().unwrap(), "--no-launch"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["agent"], "codex");
    assert_eq!(
        json["root"],
        root.canonicalize().unwrap().to_string_lossy().as_ref()
    );
    assert!(json["context"]["matches"].is_array());
    assert!(root.join(".okf-index/site/manifest.json").is_file());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn run_aliases_and_missing_client_keep_expected_behavior() {
    let root = temp();
    let alias = Command::new(env!("CARGO_BIN_EXE_okf-agent-rs"))
        .args(["run", "claude-code", root.to_str().unwrap()])
        .env("PATH", "")
        .output()
        .unwrap();
    assert_eq!(alias.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&alias.stdout).unwrap();
    assert!(json["error"].as_str().unwrap().contains("claude"));
    fs::remove_dir_all(root).unwrap();
}
