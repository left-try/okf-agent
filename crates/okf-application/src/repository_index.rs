use std::path::Path;

use okf_domain::diagnostics::Diagnostic;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DetectedFacts {
    pub languages: Vec<String>,
    pub framework_hints: Vec<String>,
    pub manifests: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDocument {
    pub path: String,
    pub content: String,
}

pub trait SourceCatalog {
    fn detect(&self, root: &Path) -> Result<DetectedFacts, Diagnostic>;
    fn discover(&self, root: &Path) -> Result<Vec<SourceDocument>, Diagnostic>;
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct IndexReport {
    pub files: usize,
    pub symbols: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub path: String,
    pub excerpt: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IndexedDocument {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IndexedSymbol {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportEdge {
    pub source: String,
    pub target: String,
    pub kind: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct IndexedContent {
    pub documents: Vec<IndexedDocument>,
    pub symbols: Vec<IndexedSymbol>,
    pub edges: Vec<ImportEdge>,
}

pub trait IndexStore {
    fn rebuild(&self, root: &Path, documents: &[SourceDocument])
    -> Result<IndexReport, Diagnostic>;
    fn search(&self, root: &Path, query: &str, limit: usize) -> Result<Vec<SearchHit>, Diagnostic>;
    fn indexed_content(&self, root: &Path) -> Result<IndexedContent, Diagnostic>;
}

pub fn build(
    root: &Path,
    catalog: &impl SourceCatalog,
    store: &impl IndexStore,
) -> Result<IndexReport, Diagnostic> {
    let documents = catalog.discover(root)?;
    store.rebuild(root, &documents)
}

pub fn search(
    root: &Path,
    query: &str,
    limit: usize,
    store: &impl IndexStore,
) -> Result<Vec<SearchHit>, Diagnostic> {
    store.search(root, query, limit)
}

pub fn indexed_content(root: &Path, store: &impl IndexStore) -> Result<IndexedContent, Diagnostic> {
    store.indexed_content(root)
}
