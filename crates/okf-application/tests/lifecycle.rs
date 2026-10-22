use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use okf_application::{
    lifecycle::{self, GitPort, InstallReport, LifecycleFs, ValidationReport},
    repository_index::{
        DetectedFacts, IndexReport, IndexStore, IndexedContent, SearchHit, SourceCatalog,
        SourceDocument,
    },
};
use okf_domain::diagnostics::{Diagnostic, ErrorKind};

#[derive(Default)]
struct Fs {
    writes: Mutex<Vec<(String, String)>>,
}
impl LifecycleFs for Fs {
    fn initialize_files(&self, _: &Path, _: &[String]) -> Result<Vec<String>, Diagnostic> {
        Ok(vec![".okf/manifest.yaml".into()])
    }
    fn replace_generated(&self, _: &Path, path: &str, body: &str) -> Result<(), Diagnostic> {
        self.writes.lock().unwrap().push((path.into(), body.into()));
        Ok(())
    }
    fn install_adapters(
        &self,
        _: &Path,
        _: Option<&Path>,
        _: bool,
    ) -> Result<InstallReport, Diagnostic> {
        Ok(InstallReport {
            installed: vec!["AGENTS.md".into()],
            conflicts: vec![],
        })
    }
    fn validate_files(&self, _: &Path) -> Result<ValidationReport, Diagnostic> {
        Ok(ValidationReport {
            ok: true,
            ..Default::default()
        })
    }
    fn record_decision(
        &self,
        _: &Path,
        _: &str,
        _: &str,
        _: &[String],
    ) -> Result<String, Diagnostic> {
        Ok(".okf/decisions/example.md".into())
    }
    fn install_codex_skill(&self, _: Option<&Path>) -> Result<String, Diagnostic> {
        Ok("skills/okf-repository-brain".into())
    }
}
struct Git;
impl GitPort for Git {
    fn changed_files(&self, _: &Path) -> Vec<String> {
        vec!["src/main.rs".into()]
    }
    fn git_dir(&self, _: &Path) -> Option<PathBuf> {
        None
    }
}
struct Catalog;
impl SourceCatalog for Catalog {
    fn detect(&self, _: &Path) -> Result<DetectedFacts, Diagnostic> {
        Ok(DetectedFacts {
            languages: vec!["Rust".into()],
            manifests: vec!["Cargo.toml".into()],
            framework_hints: vec![],
        })
    }
    fn discover(&self, _: &Path) -> Result<Vec<SourceDocument>, Diagnostic> {
        Ok(vec![SourceDocument {
            path: "src/main.rs".into(),
            content: "fn main() {}".into(),
        }])
    }
}
struct Store;
impl IndexStore for Store {
    fn rebuild(&self, _: &Path, docs: &[SourceDocument]) -> Result<IndexReport, Diagnostic> {
        Ok(IndexReport {
            files: docs.len(),
            symbols: 1,
        })
    }
    fn search(&self, _: &Path, _: &str, _: usize) -> Result<Vec<SearchHit>, Diagnostic> {
        Ok(vec![])
    }
    fn indexed_content(&self, _: &Path) -> Result<IndexedContent, Diagnostic> {
        Err(Diagnostic::new(ErrorKind::NotFound, "no index"))
    }
}

#[test]
fn initialize_and_update_refresh_generated_facts_and_index_through_ports() {
    let fs = Fs::default();
    let root = Path::new(".");
    let initialized = lifecycle::initialize(root, &[], &fs, &Git, &Catalog, &Store).unwrap();
    assert_eq!(initialized.created, vec![".okf/manifest.yaml"]);
    assert_eq!(initialized.index.files, 1);
    assert_eq!(initialized.adapter_conflicts, Vec::<String>::new());
    let update = lifecycle::update(
        root,
        Some(&["README.md".into()]),
        &fs,
        &Git,
        &Catalog,
        &Store,
    )
    .unwrap();
    assert_eq!(update.changed_files, vec!["README.md"]);
    assert_eq!(update.facts.languages, vec!["Rust"]);
    assert_eq!(update.index.files, 1);
    assert!(
        fs.writes
            .lock()
            .unwrap()
            .iter()
            .any(|(path, body)| path == ".okf/changes/current.md" && body.contains("README.md"))
    );
}

#[test]
fn validate_keeps_missing_index_as_a_warning() {
    let report = lifecycle::validate(Path::new("."), &Fs::default(), &Catalog, &Store).unwrap();
    assert!(report.ok);
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("Index is absent"))
    );
}
