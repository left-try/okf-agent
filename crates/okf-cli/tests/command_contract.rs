use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_okf-agent"))
        .args(args)
        .output()
        .expect("run okf-agent")
}

#[test]
fn version_uses_the_rust_binary_name_and_workspace_version() {
    let output = run(&["--version"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "okf-agent 0.2.1"
    );
}

#[test]
fn help_lists_the_initial_capability_command() {
    let output = run(&["--help"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    let usage = stdout
        .lines()
        .find(|line| line.starts_with("Usage: "))
        .expect("help contains usage line");
    let executable = usage
        .strip_prefix("Usage: ")
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .trim_end_matches(".exe");
    assert_eq!(executable, "okf-agent");
    assert!(stdout.contains("capabilities"));
}

#[test]
fn help_exposes_the_documented_cli_commands_and_init_profiles() {
    let output = run(&["--help"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("docs"));
    assert!(stdout.contains("bundle"));
    let init = run(&["init", "--help"]);
    assert!(String::from_utf8_lossy(&init.stdout).contains("--profiles"));
    let skills = run(&["skills", "--help"]);
    assert!(String::from_utf8_lossy(&skills.stdout).contains("doctor"));
}

#[test]
fn capabilities_reports_only_the_rust_commands_that_are_ready() {
    let output = run(&["capabilities"]);

    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let implemented = result["implemented"].as_array().unwrap();
    assert!(implemented.iter().any(|command| command == "skills apply"));
    assert!(implemented.iter().any(|command| command == "skills doctor"));
    assert_eq!(
        implemented.iter().any(|command| command == "mcp"),
        cfg!(feature = "mcp")
    );
}

#[test]
fn unported_command_returns_structured_json_and_exit_code_two() {
    let output = run(&["future-command"]);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "{\"error\":{\"code\":\"not_implemented\",\"message\":\"Rust command \\\"future-command\\\" is not implemented yet; use `okf-agent`.\"}}\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn cached_update_notice_goes_to_stderr_without_corrupting_json() {
    let root = std::env::temp_dir().join(format!("okf-update-notice-{}", std::process::id()));
    std::fs::create_dir_all(root.join("okf-agent")).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    std::fs::write(
        root.join("okf-agent/release-check.json"),
        serde_json::json!({"checked_at":now,"response":"{\"tag_name\":\"v999.0.0\"}"}).to_string(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_okf-agent"))
        .args(["index", root.to_str().unwrap()])
        .env("LOCALAPPDATA", &root)
        .env("XDG_CACHE_HOME", &root)
        .env_remove("OKF_DISABLE_UPDATE_CHECK")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok());
    assert!(String::from_utf8_lossy(&output.stderr).contains("999.0.0"));
    let _ = std::fs::remove_dir_all(root);
}
