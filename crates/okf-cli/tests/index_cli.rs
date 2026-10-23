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
    let output = Command::new(env!("CARGO_BIN_EXE_okf-agent"))
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
    assert!(root.join(".okf/manifest.yaml").is_file());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn index_from_a_nested_directory_uses_the_git_repository_root() {
    let root = std::env::temp_dir().join(format!(
        "okf-index-nested-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let nested = root.join("packages/app");
    fs::create_dir_all(&nested).unwrap();
    fs::write(root.join("lib.rs"), "pub struct IndexedThing;").unwrap();
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&root)
        .status()
        .unwrap();
    assert!(init.success());

    let output = Command::new(env!("CARGO_BIN_EXE_okf-agent"))
        .arg("index")
        .arg(&nested)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(root.join(".okf-index/knowledge.sqlite3").is_file());
    assert!(root.join(".okf/manifest.yaml").is_file());
    assert!(!nested.join(".okf-index").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn index_refuses_to_follow_an_escaping_index_directory_symlink() {
    let root = std::env::temp_dir().join(format!(
        "okf-index-symlink-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let outside = root.with_extension("outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let link = root.join(".okf-index");
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_dir(&outside, &link);
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&outside, &link);
    #[cfg(not(any(windows, unix)))]
    let linked: std::io::Result<()> = Err(std::io::Error::other("directory symlinks unsupported"));
    if linked.is_err() {
        eprintln!("skipping symlink escape assertion because symlink creation is unavailable");
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
        return;
    }

    let output = Command::new(env!("CARGO_BIN_EXE_okf-agent"))
        .arg("index")
        .arg(&root)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(!outside.join("knowledge.sqlite3").exists());
    assert!(!root.join(".okf/manifest.yaml").exists());
    #[cfg(windows)]
    fs::remove_dir(link).unwrap();
    #[cfg(unix)]
    fs::remove_file(link).unwrap();
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}
