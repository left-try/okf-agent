use std::{fs, process::Command};

#[test]
fn index_command_builds_a_local_sqlite_index_and_reports_counts() {
    let root = std::env::temp_dir().join(format!(
        "okf-index-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("lib.rs"), "pub struct IndexedThing;").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_okf-agent-rs"))
        .arg("index")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["files"], 1);
    assert_eq!(report["symbols"], 1);
    assert!(root.join(".okf-index/knowledge.sqlite3").is_file());
    fs::remove_dir_all(root).unwrap();
}
