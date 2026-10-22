use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_okf-agent-rs"))
        .args(args)
        .output()
        .expect("run okf-agent-rs")
}

#[test]
fn version_uses_the_rust_binary_name_and_workspace_version() {
    let output = run(&["--version"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "okf-agent-rs 0.1.1"
    );
}

#[test]
fn help_lists_the_initial_capability_command() {
    let output = run(&["--help"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("okf-agent-rs"));
    assert!(stdout.contains("capabilities"));
}

#[test]
fn capabilities_reports_only_the_rust_commands_that_are_ready() {
    let output = run(&["capabilities"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "{\"implemented\":[\"capabilities\"]}\n"
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
