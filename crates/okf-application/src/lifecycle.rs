use std::path::Path;

use okf_domain::diagnostics::Diagnostic;
use serde::{Deserialize, Serialize};

use crate::repository_index::{self, DetectedFacts, IndexReport, IndexStore, SourceCatalog};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub ok: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct InitReport {
    pub created: Vec<String>,
    pub artifacts: Vec<String>,
    pub index: IndexReport,
    pub adapter_conflicts: Vec<String>,
    pub docs: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UpdateReport {
    pub changed_files: Vec<String>,
    pub facts: DetectedFacts,
    pub index: IndexReport,
    pub docs: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct InstallReport {
    pub installed: Vec<String>,
    pub conflicts: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub root: String,
    pub git: bool,
    pub client: serde_json::Value,
    pub validation: ValidationReport,
}

pub trait GitPort {
    fn changed_files(&self, root: &Path) -> Vec<String>;
    fn git_dir(&self, root: &Path) -> Option<std::path::PathBuf>;
    fn has_git(&self, root: &Path) -> bool {
        self.git_dir(root).is_some()
    }
}
pub trait LifecycleFs {
    fn initialize_files(&self, root: &Path, profiles: &[String])
    -> Result<Vec<String>, Diagnostic>;
    fn replace_generated(&self, root: &Path, relative: &str, body: &str) -> Result<(), Diagnostic>;
    fn install_adapters(
        &self,
        root: &Path,
        git_dir: Option<&Path>,
        workflow_profiles: bool,
    ) -> Result<InstallReport, Diagnostic>;
    fn validate_files(&self, root: &Path) -> Result<ValidationReport, Diagnostic>;
    fn record_decision(
        &self,
        root: &Path,
        title: &str,
        rationale: &str,
        sources: &[String],
    ) -> Result<String, Diagnostic>;
    fn install_codex_skill(&self, skills_dir: Option<&Path>) -> Result<String, Diagnostic>;
}

pub fn initialize(
    root: &Path,
    profiles: &[String],
    fs: &impl LifecycleFs,
    git: &impl GitPort,
    catalog: &impl SourceCatalog,
    index: &impl IndexStore,
) -> Result<InitReport, Diagnostic> {
    let created = fs.initialize_files(root, profiles)?;
    let index_report = repository_index::build(root, catalog, index)?;
    let facts = catalog.detect(root)?;
    refresh_generated(root, &git.changed_files(root), &facts, fs)?;
    let git_dir = git.git_dir(root);
    let installed = fs.install_adapters(root, git_dir.as_deref(), !profiles.is_empty())?;
    Ok(InitReport {
        created,
        artifacts: installed.installed,
        index: index_report,
        adapter_conflicts: installed.conflicts,
        docs: None,
    })
}

pub fn update(
    root: &Path,
    changed: Option<&[String]>,
    fs: &impl LifecycleFs,
    git: &impl GitPort,
    catalog: &impl SourceCatalog,
    index: &impl IndexStore,
) -> Result<UpdateReport, Diagnostic> {
    fs.initialize_files(root, &[])?;
    let changed_files = changed
        .map(<[String]>::to_vec)
        .unwrap_or_else(|| git.changed_files(root));
    let facts = catalog.detect(root)?;
    refresh_generated(root, &changed_files, &facts, fs)?;
    let index = repository_index::build(root, catalog, index)?;
    Ok(UpdateReport {
        changed_files,
        facts,
        index,
        docs: None,
    })
}

pub fn validate(
    root: &Path,
    fs: &impl LifecycleFs,
    catalog: &impl SourceCatalog,
    index: &impl IndexStore,
) -> Result<ValidationReport, Diagnostic> {
    let mut report = fs.validate_files(root)?;
    match index.indexed_content(root) {
        Err(error) if error.kind == okf_domain::diagnostics::ErrorKind::NotFound => report
            .warnings
            .push("Index is absent; run `okf-agent index`".to_owned()),
        Err(_) => report
            .warnings
            .push("Index is stale; run `okf-agent index` or `okf-agent update`".to_owned()),
        Ok(indexed) => match catalog.discover(root) {
            Ok(current) => {
                let matches = indexed.documents.len() == current.len()
                    && indexed
                        .documents
                        .iter()
                        .zip(current.iter())
                        .all(|(a, b)| a.path == b.path && a.content == b.content);
                if !matches {
                    report.warnings.push(
                        "Index is stale; run `okf-agent index` or `okf-agent update`".to_owned(),
                    )
                }
            }
            Err(_) => report
                .warnings
                .push("Index freshness could not be checked".to_owned()),
        },
    }
    report.ok = report.errors.is_empty();
    Ok(report)
}

pub fn record_decision(
    root: &Path,
    title: &str,
    rationale: &str,
    sources: &[String],
    fs: &impl LifecycleFs,
) -> Result<String, Diagnostic> {
    fs.record_decision(root, title, rationale, sources)
}

pub fn install(
    root: &Path,
    git_dir: Option<&Path>,
    workflow_profiles: bool,
    fs: &impl LifecycleFs,
) -> Result<InstallReport, Diagnostic> {
    fs.install_adapters(root, git_dir, workflow_profiles)
}

pub fn install_codex_skill(
    skills_dir: Option<&Path>,
    fs: &impl LifecycleFs,
) -> Result<String, Diagnostic> {
    fs.install_codex_skill(skills_dir)
}

fn refresh_generated(
    root: &Path,
    changed: &[String],
    facts: &DetectedFacts,
    fs: &impl LifecycleFs,
) -> Result<(), Diagnostic> {
    let languages = if facts.languages.is_empty() {
        "unknown".to_owned()
    } else {
        facts.languages.join(", ")
    };
    let frameworks = if facts.framework_hints.is_empty() {
        "none".to_owned()
    } else {
        facts.framework_hints.join(", ")
    };
    let manifests = if facts.manifests.is_empty() {
        "none".to_owned()
    } else {
        facts.manifests.join(", ")
    };
    fs.replace_generated(
        root,
        ".okf/application/overview.md",
        &format!("## Generated facts\n- Languages: {languages}\n- Framework hints: {frameworks}"),
    )?;
    fs.replace_generated(
        root,
        ".okf/architecture/index.md",
        &format!("## Generated facts\n- Manifests: {manifests}"),
    )?;
    let files = if changed.is_empty() {
        "- No Git diff detected".to_owned()
    } else {
        changed
            .iter()
            .map(|path| format!("- `{path}`"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    fs.replace_generated(
        root,
        ".okf/changes/current.md",
        &format!("Changed files:\n{files}"),
    )?;
    Ok(())
}
