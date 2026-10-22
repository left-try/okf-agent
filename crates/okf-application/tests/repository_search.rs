use std::path::Path;

use okf_application::repository_index::{
    self, DetectedFacts, ImportEdge, IndexReport, IndexStore, IndexedContent, SearchHit,
    SourceCatalog, SourceDocument,
};
use okf_domain::diagnostics::Diagnostic;

struct Catalog;
impl SourceCatalog for Catalog {
    fn detect(&self, _root: &Path) -> Result<DetectedFacts, Diagnostic> {
        Ok(DetectedFacts {
            languages: vec!["Rust".into()],
            ..Default::default()
        })
    }
    fn discover(&self, _root: &Path) -> Result<Vec<SourceDocument>, Diagnostic> {
        Ok(vec![SourceDocument {
            path: "src/lib.rs".into(),
            content: "pub fn marker() {}".into(),
        }])
    }
}
struct Store;
impl IndexStore for Store {
    fn rebuild(
        &self,
        _root: &Path,
        documents: &[SourceDocument],
    ) -> Result<IndexReport, Diagnostic> {
        Ok(IndexReport {
            files: documents.len(),
            symbols: 1,
        })
    }
    fn search(
        &self,
        _root: &Path,
        query: &str,
        _limit: usize,
    ) -> Result<Vec<SearchHit>, Diagnostic> {
        Ok(vec![SearchHit {
            path: "src/lib.rs".into(),
            excerpt: query.into(),
        }])
    }
    fn indexed_content(&self, _root: &Path) -> Result<IndexedContent, Diagnostic> {
        Ok(IndexedContent {
            edges: vec![ImportEdge {
                source: "a.rs".into(),
                target: "b".into(),
                kind: "import".into(),
            }],
            ..Default::default()
        })
    }
}

#[test]
fn use_cases_connect_discovery_and_store_ports() {
    let root = Path::new(".");
    let report = repository_index::build(root, &Catalog, &Store).unwrap();
    assert_eq!(report.files, 1);
    assert_eq!(
        repository_index::search(root, "marker", 5, &Store).unwrap()[0].excerpt,
        "marker"
    );
    assert_eq!(
        repository_index::indexed_content(root, &Store)
            .unwrap()
            .edges[0]
            .target,
        "b"
    );
}
