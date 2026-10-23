use std::sync::atomic::{AtomicUsize, Ordering};
use std::{fs, path::PathBuf};

use okf_adapter_sqlite::bundle::SqliteBundleStore;
use okf_application::bundle::{
    BundleIndexReport, BundleLink, BundlePage, BundleProvenance, BundleStore, PreparedBundle,
};
use serde_json::json;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "okf-bundle-sqlite-{}-{}-{}",
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

fn prepared(body: &str) -> PreparedBundle {
    PreparedBundle {
        pages: vec![
            BundlePage {
                path: "index.md".into(),
                title: "Course".into(),
                doc_type: "".into(),
                aliases: vec![],
                frontmatter: json!({}),
                body: "# Course\n".into(),
                content_hash: "hash-index".into(),
            },
            BundlePage {
                path: "lesson.md".into(),
                title: "Lesson".into(),
                doc_type: "lesson".into(),
                aliases: vec!["intro".into()],
                frontmatter: json!({"type":"lesson","unknown":"kept"}),
                body: body.into(),
                content_hash: format!("hash-{body}"),
            },
        ],
        links: vec![BundleLink {
            source: "index.md".into(),
            target: Some("lesson.md".into()),
            locator: "lesson.md".into(),
            relation: Some("includes".into()),
            status: "resolved".into(),
            diagnostic: None,
        }],
        provenance: vec![BundleProvenance {
            document: "lesson.md".into(),
            source: json!({"resource":"https://example.test/source"}),
        }],
        diagnostics: vec![],
        conformant: true,
    }
}

#[test]
fn bundle_index_search_graph_export_and_revisions_preserve_schema_contract() {
    let scratch = Scratch::new();
    let store = SqliteBundleStore;
    let report = store
        .rebuild(
            &scratch.0,
            "/bundle",
            &prepared("searchable curriculum text"),
        )
        .unwrap();
    let report: BundleIndexReport = report;
    assert_eq!(report.documents, 2);
    assert!(report.conformant);
    assert_eq!(
        store.search(&scratch.0, "curriculum", 10).unwrap()[0].path,
        "lesson.md"
    );
    let graph = store.graph(&scratch.0, "index.md", 1, false).unwrap();
    assert_eq!(graph["edges"][0]["relation"], "includes");
    let export = store.export(&scratch.0).unwrap();
    assert_eq!(export["profile_contract"]["okf_version"], "0.2");
    assert_eq!(export["nodes"][1]["frontmatter"]["unknown"], "kept");
    assert_eq!(export["provenance"][0]["document"], "lesson.md");
    store
        .rebuild(
            &scratch.0,
            "/bundle",
            &prepared("changed searchable curriculum text"),
        )
        .unwrap();
    assert_eq!(store.export(&scratch.0).unwrap()["nodes"][1]["revision"], 2);
}

#[test]
fn backlinks_traverse_only_resolved_incoming_edges() {
    let scratch = Scratch::new();
    let store = SqliteBundleStore;
    store
        .rebuild(&scratch.0, "/bundle", &prepared("body"))
        .unwrap();
    let graph = store.graph(&scratch.0, "lesson.md", 1, true).unwrap();
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(graph["edges"][0]["source"], "index.md");
}
