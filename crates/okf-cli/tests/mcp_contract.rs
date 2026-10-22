use serde_json::Value;
use std::process::Command;

#[test]
fn default_binary_reports_actionable_optional_feature_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_okf-agent-rs"))
        .args(["mcp", "--profile", "repository"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let body: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(body["error"]["code"], "feature_disabled");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--features mcp")
    );
}

#[test]
fn bundle_profile_requires_both_explicit_roots() {
    let output = Command::new(env!("CARGO_BIN_EXE_okf-agent-rs"))
        .args(["mcp", "--profile", "bundle"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--root and --index-root"));
}
