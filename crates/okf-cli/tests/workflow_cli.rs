use std::process::Command;

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
