use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use okf_application::docs::{CheckReport, DocsPort, DocsReport};
use okf_application::repository_index::{DetectedFacts, IndexedContent, SourceCatalog};
use okf_domain::diagnostics::{Diagnostic, ErrorKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MANIFEST: &str = "manifest.json";

#[derive(Serialize, Deserialize)]
struct Manifest {
    schema_version: u32,
    sources: BTreeMap<String, String>,
    outputs: BTreeMap<String, String>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn io(path: &Path, error: std::io::Error) -> Diagnostic {
    Diagnostic::new(ErrorKind::Io, error.to_string()).at_path(path.to_string_lossy().to_string())
}

fn has_symlink_component(root: &Path, relative: &Path) -> bool {
    let mut cursor = root.to_path_buf();
    for component in relative {
        cursor.push(component);
        if fs::symlink_metadata(&cursor).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return true;
        }
    }
    false
}

pub fn output_path(root: &Path, output: Option<&Path>) -> Result<PathBuf, Diagnostic> {
    let root = root.canonicalize().map_err(|e| io(root, e))?;
    let candidate = output
        .map(|p| {
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                root.join(p)
            }
        })
        .unwrap_or_else(|| root.join(".okf-index/site"));
    let candidate = if candidate.is_absolute() {
        candidate
    } else {
        root.join(candidate)
    };
    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(Diagnostic::new(
            ErrorKind::InvalidPath,
            "Documentation output must stay inside the repository",
        ));
    }
    let relative = candidate.strip_prefix(&root).map_err(|_| {
        Diagnostic::new(
            ErrorKind::InvalidPath,
            "Documentation output must stay inside the repository",
        )
    })?;
    let mut cursor = root.clone();
    for part in relative {
        cursor.push(part);
        if fs::symlink_metadata(&cursor).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Diagnostic::new(
                ErrorKind::InvalidPath,
                "Documentation output cannot traverse a symlink",
            )
            .at_path(cursor.to_string_lossy().to_string()));
        }
    }
    Ok(candidate)
}

fn sources(
    root: &Path,
    catalog: &impl SourceCatalog,
) -> Result<Vec<(String, Vec<u8>)>, Diagnostic> {
    let bundle = root.join(".okf");
    if !bundle.is_dir() {
        return Err(Diagnostic::new(
            ErrorKind::NotFound,
            "Knowledge bundle is missing; run `okf-agent init` first",
        ));
    }
    let mut paths = BTreeSet::new();
    for entry in walkdir::WalkDir::new(&bundle).follow_links(false) {
        let entry = entry.map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()))?;
        if entry.file_type().is_file()
            && (entry.path().extension().is_some_and(|ext| ext == "md")
                || entry.path().ends_with("manifest.yaml")
                || entry.path().ends_with("config.json"))
        {
            paths.insert(entry.path().to_path_buf());
        }
    }
    for doc in catalog.discover(root)? {
        paths.insert(root.join(doc.path));
    }
    let mut result = Vec::new();
    for path in paths {
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| Diagnostic::new(ErrorKind::InvalidPath, "Source path escapes repository"))?
            .to_string_lossy()
            .replace('\\', "/");
        result.push((relative, fs::read(&path).map_err(|e| io(&path, e))?));
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}
fn title(path: &str, content: &str) -> String {
    content
        .lines()
        .find_map(|line| {
            line.strip_prefix("# ")
                .map(|s| s.trim().trim_matches('#').to_owned())
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            Path::new(path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .replace(['-', '_'], " ")
        })
}
fn facts_page(root: &Path, facts: &DetectedFacts) -> String {
    let list = |v: &[String], empty: &str| {
        if v.is_empty() {
            empty.to_owned()
        } else {
            v.join(", ")
        }
    };
    format!(
        "# {} Repository\n\n## Detected project facts\n- Languages: {}\n- Framework hints: {}\n- Manifests: {}\n\nThese facts are extracted from repository files. Curated explanations live in the linked knowledge pages.\n",
        root.file_name().unwrap_or_default().to_string_lossy(),
        list(&facts.languages, "unknown"),
        list(&facts.framework_hints, "none"),
        list(&facts.manifests, "none")
    )
}
fn code_map(indexed: &IndexedContent) -> String {
    let mut out = String::from("# Code Map\n\n## Indexed source files\n\n");
    let docs: Vec<_> = indexed
        .documents
        .iter()
        .filter(|d| {
            Path::new(&d.path).extension().is_some_and(|e| {
                [
                    "py", "js", "ts", "tsx", "jsx", "go", "rs", "java", "cs", "rb", "php",
                ]
                .contains(&e.to_string_lossy().as_ref())
            })
        })
        .collect();
    if docs.is_empty() {
        out.push_str("No indexed source files were found.\n")
    } else {
        for d in docs {
            out.push_str(&format!("- [`{}`]({})\n", d.path, d.path));
        }
    }
    out.push_str("\n## Indexed symbols\n\n");
    if indexed.symbols.is_empty() {
        out.push_str("No indexed symbols were found.\n")
    } else {
        for s in &indexed.symbols {
            out.push_str(&format!("- [`{}`]({}): `{}`\n", s.path, s.path, s.name));
        }
    }
    out.push_str("\n## Indexed import edges\n\n");
    if indexed.edges.is_empty() {
        out.push_str("No import edges were found.\n")
    } else {
        for e in &indexed.edges {
            out.push_str(&format!(
                "- [`{}`]({}) → `{}` ({})\n",
                e.source, e.source, e.target, e.kind
            ));
        }
    }
    out
}
fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<(), Diagnostic> {
    if !path.exists() || fs::read(path).map_err(|e| io(path, e))? != bytes {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
        }
        fs::write(path, bytes).map_err(|e| io(path, e))?;
    }
    Ok(())
}

pub fn build(
    root: &Path,
    output: Option<&Path>,
    indexed: &IndexedContent,
    catalog: &impl SourceCatalog,
) -> Result<DocsReport, Diagnostic> {
    let root = root.canonicalize().map_err(|e| io(root, e))?;
    let target = output_path(&root, output)?;
    let sources = sources(&root, catalog)?;
    let mut generated: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut pages = vec![
        (
            "repository.md".to_owned(),
            format!(
                "{} Repository",
                root.file_name().unwrap_or_default().to_string_lossy()
            ),
        ),
        ("code-map.md".to_owned(), "Code Map".into()),
    ];
    generated.insert(
        "repository.md".into(),
        facts_page(&root, &catalog.detect(&root)?).into_bytes(),
    );
    generated.insert("code-map.md".into(), code_map(indexed).into_bytes());
    for (relative, bytes) in &sources {
        if relative.ends_with(".md") {
            let page = format!("source/{relative}");
            pages.push((
                page.clone(),
                title(relative, &String::from_utf8_lossy(bytes)),
            ));
            generated.insert(page, bytes.clone());
        }
    }
    let mut index = format!(
        "# {} Repository Wiki\n\n## Pages\n\n",
        root.file_name().unwrap_or_default().to_string_lossy()
    );
    for (route, name) in &pages {
        index.push_str(&format!("- [{name}]({route})\n"));
    }
    generated.insert("index.md".into(), index.into_bytes());
    let outputs = generated
        .iter()
        .map(|(k, v)| (k.clone(), digest(v)))
        .collect::<BTreeMap<_, _>>();
    let source_hashes = sources
        .iter()
        .map(|(k, v)| (k.clone(), digest(v)))
        .collect();
    let manifest = Manifest {
        schema_version: 1,
        sources: source_hashes,
        outputs: outputs.clone(),
    };
    let previous = fs::read(target.join(MANIFEST))
        .ok()
        .and_then(|b| serde_json::from_slice::<Manifest>(&b).ok());
    if let Some(old) = previous {
        for path in old.outputs.keys() {
            if !outputs.contains_key(path) {
                let candidate = target.join(path);
                if candidate.starts_with(&target)
                    && candidate.is_file()
                    && !has_symlink_component(&target, Path::new(path))
                {
                    fs::remove_file(&candidate).map_err(|e| io(&candidate, e))?;
                }
            }
        }
    }
    for (name, bytes) in &generated {
        let destination = target.join(name);
        if !destination.starts_with(&target) {
            return Err(Diagnostic::new(
                ErrorKind::InvalidPath,
                "Generated documentation path escapes output directory",
            ));
        }
        if has_symlink_component(&target, Path::new(name)) {
            return Err(Diagnostic::new(
                ErrorKind::InvalidPath,
                "Generated documentation cannot traverse a symlink",
            )
            .at_path(name));
        }
        write_if_changed(&destination, bytes)?;
    }
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| Diagnostic::new(ErrorKind::InvalidConfiguration, e.to_string()))?;
    let mut bytes = bytes;
    bytes.push(b'\n');
    write_if_changed(&target.join(MANIFEST), &bytes)?;
    Ok(DocsReport {
        output: target.to_string_lossy().to_string(),
        pages: pages.len(),
        sources: sources.len(),
    })
}

pub fn check(root: &Path, output: Option<&Path>) -> Result<CheckReport, Diagnostic> {
    let target = match output_path(root, output) {
        Ok(target) => target,
        Err(error) => {
            return Ok(CheckReport {
                ok: false,
                errors: vec![error.message],
                output: output
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| root.join(".okf-index/site").to_string_lossy().to_string()),
            });
        }
    };
    let mut errors = Vec::new();
    let manifest_path = target.join(MANIFEST);
    let manifest:Manifest=match fs::read(&manifest_path).ok().and_then(|b|serde_json::from_slice::<Manifest>(&b).ok()){Some(m) if m.schema_version==1=>m,_=>return Ok(CheckReport{ok:false,errors:vec!["Documentation build is missing or has an invalid manifest; run `okf-agent docs build`".into()],output:target.to_string_lossy().to_string()})};
    let root = root.canonicalize().map_err(|e| io(root, e))?;
    let current = sources(&root, &crate::discovery::FsSourceCatalog)?;
    let hashes = current
        .into_iter()
        .map(|(k, v)| (k, digest(&v)))
        .collect::<BTreeMap<_, _>>();
    if hashes != manifest.sources {
        errors.push(
            "Documentation is stale because knowledge sources changed; run `okf-agent docs build`"
                .into(),
        );
    }
    for (name, expected) in &manifest.outputs {
        let path = target.join(name);
        let valid = path.starts_with(&target)
            && !has_symlink_component(&target, Path::new(name))
            && fs::read(&path).is_ok_and(|b| digest(&b) == *expected);
        if !valid {
            errors.push(format!("Generated page is missing or changed: {name}"));
        }
    }
    Ok(CheckReport {
        ok: errors.is_empty(),
        errors,
        output: target.to_string_lossy().to_string(),
    })
}

#[derive(Default)]
pub struct FsDocs;
impl DocsPort for FsDocs {
    fn build(
        &self,
        root: &Path,
        output: Option<&Path>,
        indexed: &IndexedContent,
        catalog: &impl SourceCatalog,
    ) -> Result<DocsReport, Diagnostic> {
        build(root, output, indexed, catalog)
    }
    fn check(&self, root: &Path, output: Option<&Path>) -> Result<CheckReport, Diagnostic> {
        check(root, output)
    }
}
