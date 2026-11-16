use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use okf_adapter_sqlite::repository::SqliteIndexStore;
use okf_application::repository_index::{IndexStore, SourceDocument};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "okf-sqlite-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn rebuild_preserves_schema_and_searches_documents_symbols_and_import_edges() {
    let scratch = Scratch::new();
    let store = SqliteIndexStore;
    let docs = [
        SourceDocument {
            path: "src/a.py".into(),
            content:
                "from app.core import run\ndef searchable_symbol():\n    return 'needle phrase'\n"
                    .into(),
        },
        SourceDocument {
            path: "src/b.rs".into(),
            content: "use crate::module;\nstruct RustThing;\n".into(),
        },
    ];
    let report = store.rebuild(&scratch.0, &docs).unwrap();
    assert_eq!(report.files, 2);
    assert_eq!(report.symbols, 2);
    let indexed = store.indexed_content(&scratch.0).unwrap();
    assert_eq!(indexed.documents.len(), 2);
    assert!(
        indexed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "searchable_symbol")
    );
    assert!(indexed.edges.iter().any(|edge| edge.target == "app.core"));
    assert_eq!(
        store.search(&scratch.0, "needle", 8).unwrap()[0].path,
        "src/a.py"
    );
    assert!(
        store
            .search(&scratch.0, "searchable_symbol", 8)
            .unwrap()
            .iter()
            .any(|hit| hit.excerpt == "symbol: searchable_symbol")
    );
}

#[test]
fn rebuilding_a_smaller_index_reclaims_unused_database_pages() {
    let scratch = Scratch::new();
    let store = SqliteIndexStore;
    let docs: Vec<_> = (0..24)
        .map(|i| SourceDocument {
            path: format!("cache/{i}.rs"),
            content: format!("struct Cache{i};\n{}", "indexable content ".repeat(25_000)),
        })
        .collect();
    store.rebuild(&scratch.0, &docs).unwrap();
    let path = scratch.0.join(".okf-index/knowledge.sqlite3");
    let before = fs::metadata(&path).unwrap().len();
    store
        .rebuild(
            &scratch.0,
            &[SourceDocument {
                path: "src/main.rs".into(),
                content: "struct Kept;".into(),
            }],
        )
        .unwrap();
    let after = fs::metadata(&path).unwrap().len();
    assert!(after < before / 4, "before: {before}, after: {after}");
    assert!(!store.search(&scratch.0, "Kept", 5).unwrap().is_empty());
}

#[test]
fn failed_rebuild_rolls_back_and_keeps_the_previous_index() {
    let scratch = Scratch::new();
    let store = SqliteIndexStore;
    store
        .rebuild(
            &scratch.0,
            &[SourceDocument {
                path: "old.py".into(),
                content: "old content".into(),
            }],
        )
        .unwrap();
    let failed = store.rebuild(
        &scratch.0,
        &[
            SourceDocument {
                path: "duplicate.py".into(),
                content: "first".into(),
            },
            SourceDocument {
                path: "duplicate.py".into(),
                content: "second".into(),
            },
        ],
    );
    assert!(failed.is_err());
    assert_eq!(
        store.indexed_content(&scratch.0).unwrap().documents[0].path,
        "old.py"
    );
}

#[test]
fn malformed_fts_query_falls_back_to_like_search() {
    let scratch = Scratch::new();
    let store = SqliteIndexStore;
    store
        .rebuild(
            &scratch.0,
            &[SourceDocument {
                path: "a.md".into(),
                content: "text with bracket [ marker".into(),
            }],
        )
        .unwrap();
    assert_eq!(store.search(&scratch.0, "[", 8).unwrap()[0].path, "a.md");
}

#[test]
fn rebuild_rejects_an_index_directory_symlink_that_escapes_the_repository() {
    let scratch = Scratch::new();
    let outside = Scratch::new();
    let link = scratch.0.join(".okf-index");
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_dir(&outside.0, &link);
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&outside.0, &link);
    #[cfg(not(any(windows, unix)))]
    let linked: std::io::Result<()> = Err(std::io::Error::other("directory symlinks unsupported"));
    if linked.is_err() {
        eprintln!("skipping symlink escape assertion because symlink creation is unavailable");
        return;
    }

    let error = SqliteIndexStore
        .rebuild(
            &scratch.0,
            &[SourceDocument {
                path: "source.rs".into(),
                content: "fn safe() {}".into(),
            }],
        )
        .unwrap_err();

    assert!(error.to_string().contains("outside the repository root"));
    assert!(!outside.0.join("knowledge.sqlite3").exists());
}

#[test]
fn index_extracts_rust_declarations_and_slint_without_false_positives_from_comments_strings_or_manifests() {
    let scratch = Scratch::new();
    let store = SqliteIndexStore;
    let docs = vec![
        SourceDocument {
            path: "crates/stovo-desktop-app/src/app/reducer.rs".into(),
            content: "/// Pure state transition function delegating to `stovo_application::reduce`.\npub fn reduce(state: &mut State, event: AppEvent) -> bool {\n    false\n}\n".into(),
        },
        SourceDocument {
            path: "crates/stovo-application/src/lib.rs".into(),
            content: "use stovo_domain::Model;\npub fn run() {\n    let _ = reduce(&mut state, AppEvent::UpdateComposerDraft(\"Hello from Slint native!\".to_string()));\n}\n".into(),
        },
        SourceDocument {
            path: "crates/stovo-application/src/onboarding.rs".into(),
            content: "/// Builds a `ServerProfile` from a validated normalized URL and optional family/space name.\npub struct ServerProfile;\n".into(),
        },
        SourceDocument {
            path: "crates/stovo-shared-utils/Cargo.toml".into(),
            content: "[package]\ndescription = \"Reusable pure-Rust utilities ported 1:1 from stovo-messenger/frontend/src\"\n".into(),
        },
        SourceDocument {
            path: "crates/stovo-shared-utils/src/lib.rs".into(),
            content: "//! Smaller models MUST import these functions rather than re-implementing them per screen.\npub mod utils;\npub enum Route { Home }\npub trait Handler {}\n".into(),
        },
        SourceDocument {
            path: "ui/app.slint".into(),
            content: "import { Button } from \"std-widgets.slint\";\nexport component MainWindow {}\nexport global AppTheme {}\nexport struct User {}\n".into(),
        },
    ];

    store.rebuild(&scratch.0, &docs).unwrap();
    let indexed = store.indexed_content(&scratch.0).unwrap();

    let symbol_names: Vec<&str> = indexed.symbols.iter().map(|s| s.name.as_str()).collect();
    let edge_targets: Vec<&str> = indexed.edges.iter().map(|e| e.target.as_str()).collect();

    // Verify absence of false-positive symbols and import edges (Issue 35)
    assert!(!symbol_names.contains(&"delegating"), "comments must not generate symbols");
    assert!(!edge_targets.contains(&"Slint"), "strings must not generate import edges");
    assert!(!edge_targets.contains(&"a"), "doc comments must not generate import edges");
    assert!(!edge_targets.contains(&"stovo"), "manifest prose must not generate import edges");
    assert!(!edge_targets.contains(&"these"), "doc comments must not generate import edges");

    // Verify Rust fn, enum, trait, mod symbols and use/mod edges (Issue 36)
    assert!(symbol_names.contains(&"reduce"), "Rust fn symbol must be indexed");
    assert!(symbol_names.contains(&"Route"), "Rust enum symbol must be indexed");
    assert!(symbol_names.contains(&"Handler"), "Rust trait symbol must be indexed");
    assert!(symbol_names.contains(&"utils"), "Rust mod symbol must be indexed");
    assert!(symbol_names.contains(&"ServerProfile"), "Rust struct symbol must be indexed");
    assert!(edge_targets.contains(&"stovo_domain::Model"), "Rust use edge must be indexed");
    assert!(edge_targets.contains(&"utils"), "Rust mod edge must be indexed");

    // Verify Slint symbols and imports (Issue 37)
    assert!(symbol_names.contains(&"MainWindow"), "Slint component must be indexed");
    assert!(symbol_names.contains(&"AppTheme"), "Slint global must be indexed");
    assert!(symbol_names.contains(&"User"), "Slint struct must be indexed");
    assert!(edge_targets.contains(&"std-widgets.slint"), "Slint import edge must be indexed");
}
