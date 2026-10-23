use std::{fs, path::PathBuf, process::Command};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "okf-bundle-cli-{}-{}",
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
    Command::new(env!("CARGO_BIN_EXE_okf-agent"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn bundle_commands_index_search_traverse_export_and_keep_source_read_only() {
    let scratch = Scratch::new();
    let root = scratch.0.join("bundle");
    let index = scratch.0.join("index");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("index.md"),
        "# Course\n[Lesson](lesson%20one.md#part)\n",
    )
    .unwrap();
    let source = "---\ntype: Unknown Local Type\ntitle: Intro Lesson\naliases: [intro]\nextra: retained\nsources:\n  - resource: https://example.test/source\n---\nSearchable curriculum text. [[index.md|course]]\n";
    fs::write(root.join("lesson one.md"), source).unwrap();
    let root_arg = root.to_str().unwrap();
    let index_arg = index.to_str().unwrap();
    let indexed = run(&[
        "bundle",
        "index",
        "--root",
        root_arg,
        "--index-root",
        index_arg,
        "--wikilinks",
    ]);
    assert!(
        indexed.status.success(),
        "{}",
        String::from_utf8_lossy(&indexed.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&indexed.stdout).unwrap();
    assert_eq!(report["documents"], 2, "index report: {report}");
    assert!(report["conformant"].as_bool().unwrap());
    let search = run(&[
        "bundle",
        "search",
        "curriculum",
        "--root",
        root_arg,
        "--index-root",
        index_arg,
    ]);
    let hits: serde_json::Value = serde_json::from_slice(&search.stdout).unwrap();
    assert_eq!(hits[0]["path"], "lesson one.md");
    let graph = run(&[
        "bundle",
        "neighborhood",
        "index.md",
        "--root",
        root_arg,
        "--index-root",
        index_arg,
        "--depth",
        "1",
    ]);
    let graph: serde_json::Value = serde_json::from_slice(&graph.stdout).unwrap();
    assert_eq!(
        graph["nodes"].as_array().unwrap().len(),
        2,
        "graph: {graph}"
    );
    let exported = run(&[
        "bundle",
        "export",
        "--root",
        root_arg,
        "--index-root",
        index_arg,
    ]);
    let exported: serde_json::Value = serde_json::from_slice(&exported.stdout).unwrap();
    assert_eq!(exported["nodes"][1]["frontmatter"]["extra"], "retained");
    assert_eq!(exported["provenance"][0]["document"], "lesson one.md");
    assert!(
        exported["links"]
            .as_array()
            .unwrap()
            .iter()
            .any(|link| link["relation"] == "provenance")
    );
    assert_eq!(
        fs::read_to_string(root.join("lesson one.md")).unwrap(),
        source
    );
    assert!(!root.join(".okf").exists());
    assert!(!root.join("AGENTS.md").exists());
}

#[test]
fn bundle_validation_returns_one_for_conformance_errors() {
    let scratch = Scratch::new();
    let root = scratch.0.join("bundle");
    let index = scratch.0.join("index");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("note.md"), "No frontmatter\n").unwrap();
    let output = run(&[
        "bundle",
        "validate",
        "--root",
        root.to_str().unwrap(),
        "--index-root",
        index.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!result["ok"].as_bool().unwrap());
}
