use std::{fs, path::PathBuf};

use okf_adapter_sqlite::repository::SqliteIndexStore;
use okf_application::repository_index::{IndexStore, SourceDocument};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "okf-sqlite-{}-{}",
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
