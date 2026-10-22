use crate::{
    docs::{self, DocsPort},
    lifecycle::{self, GitPort, LifecycleFs},
    repository_index::{IndexStore, SourceCatalog},
};
use okf_domain::diagnostics::Diagnostic;
use std::path::Path;

pub fn prepare(
    root: &Path,
    agent: &str,
    fs: &impl LifecycleFs,
    git: &impl GitPort,
    catalog: &impl SourceCatalog,
    index: &impl IndexStore,
    docs_port: &impl DocsPort,
) -> Result<serde_json::Value, Diagnostic> {
    lifecycle::initialize(root, &[], fs, git, catalog, index)?;
    docs::build(root, None, catalog, index, docs_port)?;
    let matches = index.search(root, "repository onboarding", 8)?;
    let context = serde_json::json!({"request":"repository onboarding","matches":matches,"concepts":[],"protocol":"Retrieve context before edits; update knowledge after edits; validate before completion."});
    let payload = serde_json::json!({"agent":agent,"root":root.to_string_lossy(),"context":context,"instructions":"Call okf.get_change_context before editing, then update and validate.","context_delivery":"Repository-native instructions carry the OKF protocol; this computed context is not injected into launched clients."});
    Ok(payload)
}
