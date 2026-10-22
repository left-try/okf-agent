use std::{fs, path::Path};

use okf_adapter_fs::docs::{build, check};
use okf_application::repository_index::{IndexedContent, SourceCatalog};
use okf_domain::diagnostics::Diagnostic;
use serde_json::Value;

#[derive(Default)]
struct Catalog;

impl SourceCatalog for Catalog {
    fn detect(
        &self,
        _root: &Path,
    ) -> Result<okf_application::repository_index::DetectedFacts, Diagnostic> {
        Ok(Default::default())
    }
    fn discover(
        &self,
        _root: &Path,
    ) -> Result<Vec<okf_application::repository_index::SourceDocument>, Diagnostic> {
        Ok(Vec::new())
    }
}

fn temp() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "okf-docs-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(path.join(".okf")).unwrap();
    path
}

#[test]
fn empty_index_build_is_deterministic_and_check_is_read_only() {
    let root = temp();
    fs::write(root.join(".okf/manifest.yaml"), "schema_version: 1\n").unwrap();
    let first = build(&root, None, &IndexedContent::default(), &Catalog).unwrap();
    let manifest_path = root.join(".okf-index/site/manifest.json");
    let before = fs::read(&manifest_path).unwrap();
    let second = build(&root, None, &IndexedContent::default(), &Catalog).unwrap();
    assert_eq!(first, second);
    assert_eq!(before, fs::read(&manifest_path).unwrap());
    let check_before = fs::metadata(&manifest_path).unwrap().modified().unwrap();
    assert!(check(&root, None).unwrap().ok);
    assert_eq!(
        check_before,
        fs::metadata(&manifest_path).unwrap().modified().unwrap()
    );
    let json: Value = serde_json::from_slice(&before).unwrap();
    assert!(json["outputs"]["index.md"].is_string());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn changed_sources_make_check_stale_without_rewriting_outputs() {
    let root = temp();
    fs::write(root.join(".okf/manifest.yaml"), "schema_version: 1\n").unwrap();
    fs::write(root.join(".okf/notes.md"), "# Curated\nKeep this.\n").unwrap();
    build(&root, None, &IndexedContent::default(), &Catalog).unwrap();
    let page = root.join(".okf-index/site/source/.okf/notes.md");
    assert_eq!(
        fs::read_to_string(&page).unwrap(),
        "# Curated\nKeep this.\n"
    );
    fs::write(root.join(".okf/notes.md"), "# Changed\n").unwrap();
    let stale = check(&root, None).unwrap();
    assert!(!stale.ok);
    assert!(stale.errors.iter().any(|error| error.contains("stale")));
    assert_eq!(
        fs::read_to_string(&page).unwrap(),
        "# Curated\nKeep this.\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn output_cannot_escape_root() {
    let root = temp();
    let report = build(
        &root,
        Some(Path::new("../outside")),
        &IndexedContent::default(),
        &Catalog,
    );
    assert!(report.is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn markdown_routes_reject_traversal_and_symlink_escapes() {
    let root = temp();
    let output = root.join("site");
    fs::create_dir_all(&output).unwrap();
    fs::write(output.join("index.md"), "# Home\n").unwrap();
    let allowed = ["index.md".to_owned()].into_iter().collect();
    assert!(okf_adapter_fs::web::resolve_route(&output, "/", &allowed).is_some());
    assert!(okf_adapter_fs::web::resolve_route(&output, "/%2e%2e/secret.md", &allowed).is_none());
    assert!(okf_adapter_fs::web::resolve_route(&output, "/%5csecret.md", &allowed).is_none());
    fs::remove_dir_all(root).unwrap();
}
