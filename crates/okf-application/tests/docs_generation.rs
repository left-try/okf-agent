use okf_application::{
    docs::{self, CheckReport, DocsPort, DocsReport},
    repository_index::{
        DetectedFacts, ImportEdge, IndexReport, IndexStore, IndexedContent, IndexedDocument,
        IndexedSymbol, SearchHit, SourceCatalog, SourceDocument,
    },
};
use okf_domain::diagnostics::Diagnostic;
use std::{cell::Cell, path::Path};

struct Catalog;
impl SourceCatalog for Catalog {
    fn detect(&self, _: &Path) -> Result<DetectedFacts, Diagnostic> {
        Ok(DetectedFacts::default())
    }
    fn discover(&self, _: &Path) -> Result<Vec<SourceDocument>, Diagnostic> {
        Ok(Vec::new())
    }
}
struct Index;
impl IndexStore for Index {
    fn rebuild(&self, _: &Path, _: &[SourceDocument]) -> Result<IndexReport, Diagnostic> {
        unreachable!()
    }
    fn search(&self, _: &Path, _: &str, _: usize) -> Result<Vec<SearchHit>, Diagnostic> {
        unreachable!()
    }
    fn indexed_content(&self, _: &Path) -> Result<IndexedContent, Diagnostic> {
        Ok(IndexedContent {
            documents: vec![IndexedDocument {
                path: "src/main.rs".into(),
                content: "fn main() {}".into(),
            }],
            symbols: vec![IndexedSymbol {
                name: "main".into(),
                path: "src/main.rs".into(),
            }],
            edges: vec![ImportEdge {
                source: "src/main.rs".into(),
                target: "std".into(),
                kind: "import".into(),
            }],
        })
    }
}
struct Docs {
    built: Cell<bool>,
    checks: Cell<usize>,
}
impl DocsPort for Docs {
    fn build(
        &self,
        _: &Path,
        _: Option<&Path>,
        indexed: &IndexedContent,
        _: &impl SourceCatalog,
    ) -> Result<DocsReport, Diagnostic> {
        assert_eq!(indexed.documents[0].path, "src/main.rs");
        self.built.set(true);
        Ok(DocsReport {
            output: "site".into(),
            pages: 2,
            sources: 1,
        })
    }
    fn check(&self, _: &Path, _: Option<&Path>) -> Result<CheckReport, Diagnostic> {
        self.checks.set(self.checks.get() + 1);
        Ok(CheckReport {
            ok: false,
            errors: vec!["stale".into()],
            output: "site".into(),
        })
    }
}

#[test]
fn docs_build_uses_indexed_content_and_check_does_not_rebuild() {
    let docs_port = Docs {
        built: Cell::new(false),
        checks: Cell::new(0),
    };
    let root = Path::new(".");
    let report = docs::build(root, None, &Catalog, &Index, &docs_port).unwrap();
    assert_eq!(report.pages, 2);
    assert!(docs_port.built.get());
    let check = docs::check(root, None, &docs_port).unwrap();
    assert!(!check.ok);
    assert_eq!(docs_port.checks.get(), 1);
}
