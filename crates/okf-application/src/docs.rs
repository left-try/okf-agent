use crate::repository_index::{self, IndexStore, IndexedContent, SourceCatalog};
use okf_domain::diagnostics::Diagnostic;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocsReport {
    pub output: String,
    pub pages: usize,
    pub sources: usize,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CheckReport {
    pub ok: bool,
    pub errors: Vec<String>,
    pub output: String,
}

pub trait DocsPort {
    fn build(
        &self,
        root: &Path,
        output: Option<&Path>,
        indexed: &IndexedContent,
        catalog: &impl SourceCatalog,
    ) -> Result<DocsReport, Diagnostic>;
    fn check(&self, root: &Path, output: Option<&Path>) -> Result<CheckReport, Diagnostic>;
}

pub fn build(
    root: &Path,
    output: Option<&Path>,
    catalog: &impl SourceCatalog,
    index: &impl IndexStore,
    docs: &impl DocsPort,
) -> Result<DocsReport, Diagnostic> {
    let indexed = repository_index::indexed_content(root, index)?;
    docs.build(root, output, &indexed, catalog)
}
pub fn check(
    root: &Path,
    output: Option<&Path>,
    docs: &impl DocsPort,
) -> Result<CheckReport, Diagnostic> {
    docs.check(root, output)
}

pub fn default_output(root: &Path) -> PathBuf {
    root.join(".okf-index/site")
}

pub fn initialize_with_docs(
    root: &Path,
    profiles: &[String],
    fs: &impl crate::lifecycle::LifecycleFs,
    git: &impl crate::lifecycle::GitPort,
    catalog: &impl SourceCatalog,
    index: &impl IndexStore,
    docs_port: &impl DocsPort,
) -> Result<crate::lifecycle::InitReport, Diagnostic> {
    let mut report = crate::lifecycle::initialize(root, profiles, fs, git, catalog, index)?;
    report.docs = Some(
        serde_json::to_value(build(root, None, catalog, index, docs_port)?).map_err(|error| {
            Diagnostic::new(
                okf_domain::diagnostics::ErrorKind::InvalidConfiguration,
                error.to_string(),
            )
        })?,
    );
    Ok(report)
}
