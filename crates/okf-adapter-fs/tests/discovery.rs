use std::{fs, path::PathBuf};

use okf_adapter_fs::discovery::FsSourceCatalog;
use okf_application::repository_index::SourceCatalog;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "okf-discovery-{}-{}",
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

#[test]
fn discovers_rust_and_manifests_in_sorted_order_and_skips_generated_directories() {
    let scratch = Scratch::new();
    fs::write(scratch.0.join("Cargo.toml"), "[package]").unwrap();
    fs::write(scratch.0.join("z.rs"), "fn zed() {}").unwrap();
    fs::write(scratch.0.join("a.py"), "def alpha(): pass").unwrap();
    for dir in [
        ".git",
        ".okf",
        ".okf-index",
        "node_modules",
        ".venv",
        "venv",
        "dist",
        "build",
        "__pycache__",
    ] {
        fs::create_dir_all(scratch.0.join(dir)).unwrap();
        fs::write(scratch.0.join(dir).join("hidden.py"), "def hidden(): pass").unwrap();
    }

    let catalog = FsSourceCatalog;
    let documents = catalog.discover(&scratch.0).unwrap();
    assert_eq!(
        documents
            .iter()
            .map(|doc| doc.path.as_str())
            .collect::<Vec<_>>(),
        ["Cargo.toml", "a.py", "z.rs"]
    );
    let facts = catalog.detect(&scratch.0).unwrap();
    assert_eq!(facts.languages, ["Rust"]);
}

#[test]
fn empty_repository_has_no_indexable_sources() {
    let scratch = Scratch::new();
    assert!(FsSourceCatalog.discover(&scratch.0).unwrap().is_empty());
}

#[test]
fn truncates_at_500_000_unicode_characters_without_splitting_utf8() {
    let scratch = Scratch::new();
    let expected = format!("{}{}", "é".repeat(499_999), "🦀");
    fs::write(scratch.0.join("large.rs"), format!("{expected}ignored")).unwrap();

    let documents = FsSourceCatalog.discover(&scratch.0).unwrap();

    assert_eq!(documents[0].content, expected);
}
