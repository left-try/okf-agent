use std::path::Path;

use okf_application::repository_index::{DetectedFacts, SourceCatalog, SourceDocument};
use okf_domain::diagnostics::{Diagnostic, ErrorKind};
use walkdir::{DirEntry, WalkDir};

const SKIP: &[&str] = &[
    ".git",
    ".okf",
    ".okf-index",
    "node_modules",
    ".venv",
    "venv",
    "dist",
    "build",
    "__pycache__",
];
const SUFFIXES: &[&str] = &[
    "py", "js", "ts", "tsx", "jsx", "go", "rs", "java", "cs", "rb", "php",
];
const MANIFESTS: &[&str] = &[
    "pyproject.toml",
    "requirements.txt",
    "package.json",
    "go.mod",
    "Cargo.toml",
    "pom.xml",
    "Dockerfile",
    "docker-compose.yml",
    ".env.example",
];

#[derive(Default)]
pub struct FsSourceCatalog;

impl SourceCatalog for FsSourceCatalog {
    fn detect(&self, root: &Path) -> Result<DetectedFacts, Diagnostic> {
        let mut names = Vec::new();
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(allowed)
        {
            let entry = entry.map_err(|error| Diagnostic::new(ErrorKind::Io, error.to_string()))?;
            if entry.file_type().is_file() {
                names.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        names.sort();
        let mut facts = DetectedFacts::default();
        for (manifest, language, framework) in [
            ("pyproject.toml", "Python", None),
            ("requirements.txt", "Python", None),
            ("package.json", "JavaScript", Some("Node.js")),
            ("go.mod", "Go", None),
            ("Cargo.toml", "Rust", None),
            ("pom.xml", "Java", None),
        ] {
            if names.iter().any(|name| name == manifest) {
                facts.languages.push(language.to_owned());
                facts.manifests.push(manifest.to_owned());
                if let Some(framework) = framework {
                    facts.framework_hints.push(framework.to_owned());
                }
            }
        }
        facts.languages.sort();
        facts.languages.dedup();
        facts.manifests.sort();
        facts.manifests.dedup();
        facts.framework_hints.sort();
        facts.framework_hints.dedup();
        Ok(facts)
    }

    fn discover(&self, root: &Path) -> Result<Vec<SourceDocument>, Diagnostic> {
        let mut files = Vec::new();
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(allowed)
        {
            let entry = entry.map_err(|error| Diagnostic::new(ErrorKind::Io, error.to_string()))?;
            if !entry.file_type().is_file() || !is_indexed(entry.path()) {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|error| Diagnostic::new(ErrorKind::InvalidPath, error.to_string()))?;
            let bytes = std::fs::read(entry.path()).map_err(|error| {
                Diagnostic::new(ErrorKind::Io, error.to_string())
                    .at_path(relative.to_string_lossy().replace('\\', "/"))
            })?;
            let content = String::from_utf8_lossy(&bytes).into_owned();
            let content = content.chars().take(500_000).collect();
            files.push(SourceDocument {
                path: relative.to_string_lossy().replace('\\', "/"),
                content,
            });
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(files)
    }
}

fn allowed(entry: &DirEntry) -> bool {
    entry.depth() == 0
        || (!entry.file_type().is_dir()
            || !SKIP.contains(&entry.file_name().to_string_lossy().as_ref()))
}

fn is_indexed(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| MANIFESTS.contains(&name))
        || path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| SUFFIXES.contains(&extension.to_ascii_lowercase().as_str()))
}
