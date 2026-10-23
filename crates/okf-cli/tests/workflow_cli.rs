use std::{fs, path::PathBuf, process::Command};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "okf-workflow-root-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_okf-agent-rs"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn workflow_show_and_resolve_emit_json_and_report_readiness() {
    let dir = std::env::temp_dir();
    let show = run(&["workflow", "show", dir.to_str().unwrap()]);
    assert!(
        show.status.success(),
        "{}",
        String::from_utf8_lossy(&show.stderr)
    );
    let policy: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(policy["schema_version"], 1);

    let resolve = run(&[
        "workflow",
        "resolve",
        dir.to_str().unwrap(),
        "--type",
        "feature",
        "--risk",
        "strict",
    ]);
    assert!(
        resolve.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve.stderr)
    );
    let decision: serde_json::Value = serde_json::from_slice(&resolve.stdout).unwrap();
    assert_eq!(decision["primary_type"], "feature");
    assert_eq!(decision["rigor"], "strict");
}

#[test]
fn workflow_parser_rejects_invalid_values_with_usage_exit_code() {
    let output = run(&["workflow", "resolve", ".", "--type", "unknown"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn workflow_commands_from_nested_directories_load_policy_from_repository_root() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    let nested = root.join("packages/app");
    fs::create_dir_all(root.join(".okf/workflows")).unwrap();
    fs::create_dir_all(&nested).unwrap();
    fs::write(
        root.join(".okf/workflows/config.json"),
        r#"{"schema_version":1,"default_rigor":"light"}"#,
    )
    .unwrap();
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(root)
            .status()
            .unwrap()
            .success()
    );

    let show = run(&["workflow", "show", nested.to_str().unwrap()]);
    assert!(show.status.success());
    let policy: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(policy["default_rigor"], "light");

    let resolve = run(&[
        "workflow",
        "resolve",
        nested.to_str().unwrap(),
        "--type",
        "feature",
    ]);
    assert!(resolve.status.success());
    let decision: serde_json::Value = serde_json::from_slice(&resolve.stdout).unwrap();
    assert_eq!(decision["rigor"], "light");
}
