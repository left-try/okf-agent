use std::{fs, path::PathBuf, process::Command};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "okf-lifecycle-cli-{}-{}",
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
fn update_then_validate_refreshes_repository_files_and_index() {
    let scratch = Scratch::new();
    fs::write(scratch.0.join("Cargo.toml"), "[package]\nname='sample'\n").unwrap();
    fs::write(scratch.0.join("main.rs"), "fn main() {}\n").unwrap();
    let root = scratch.0.to_str().unwrap();
    let update = run(&["update", root, "--changed-files", "main.rs"]);
    assert!(
        update.status.success(),
        "{}",
        String::from_utf8_lossy(&update.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&update.stdout).unwrap();
    assert_eq!(result["changed_files"][0], "main.rs");
    assert_eq!(result["facts"]["languages"][0], "Rust");
    assert!(scratch.0.join(".okf/changes/current.md").is_file());
    assert!(scratch.0.join(".okf-index/knowledge.sqlite3").is_file());
    let validate = run(&["validate", root]);
    assert!(
        validate.status.success(),
        "{}",
        String::from_utf8_lossy(&validate.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&validate.stdout).unwrap();
    assert!(result["ok"].as_bool().unwrap());
}

#[test]
fn init_builds_knowledge_index_docs_and_optional_profiles() {
    let scratch = Scratch::new();
    fs::write(scratch.0.join("src.rs"), "fn sample() {}\n").unwrap();
    let root = scratch.0.to_str().unwrap();
    let output = run(&["init", root, "--profiles", "sdd,tdd"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        result["root"],
        scratch.0.canonicalize().unwrap().to_string_lossy().as_ref()
    );
    assert!(result["index"]["files"].as_u64().unwrap() > 0);
    assert!(result["docs"]["pages"].as_u64().unwrap() > 2);
    assert!(result["artifacts"].is_array());
    assert!(scratch.0.join(".okf/workflows/config.json").is_file());
    assert!(scratch.0.join(".okf-index/site/manifest.json").is_file());
}

#[test]
fn install_and_record_decision_return_machine_readable_paths() {
    let scratch = Scratch::new();
    let skills = scratch.0.join("skills");
    let install = run(&["install", "--skills-dir", skills.to_str().unwrap()]);
    assert!(install.status.success());
    let report: serde_json::Value = serde_json::from_slice(&install.stdout).unwrap();
    let skill = PathBuf::from(report["skill"].as_str().unwrap());
    assert!(skill.join("SKILL.md").is_file());
    let decision = run(&[
        "record-decision",
        scratch.0.to_str().unwrap(),
        "--title",
        "Use Rust",
        "--rationale",
        "Native implementation",
    ]);
    assert!(decision.status.success());
    let report: serde_json::Value = serde_json::from_slice(&decision.stdout).unwrap();
    let path = report["path"].as_str().unwrap();
    assert!(scratch.0.join(path).is_file());
    assert!(
        fs::read_to_string(scratch.0.join(path))
            .unwrap()
            .contains("Native implementation")
    );
}
