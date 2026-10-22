//! Filesystem, template, and serialization adapters.

pub mod bundle;
pub mod discovery;
pub mod docs;
pub mod frontmatter;
pub mod generated_blocks;
pub mod instructions;
pub mod lifecycle;
pub mod markdown_links;
pub mod paths;
pub mod templates;
pub mod web;
pub mod yaml;

use std::fs;
use std::path::{Path, PathBuf};

use okf_domain::diagnostics::{Diagnostic, ErrorKind};
use okf_domain::paths::RepoPath;
use okf_domain::workflow::WorkflowPolicy;

pub fn load_workflow_policy(root: &Path) -> Result<WorkflowPolicy, Diagnostic> {
    let relative = PathBuf::from(".okf/workflows/config.json");
    let path = RepoPath::new(root, &relative)?;
    let absolute = root.join(path.as_path());
    let source = match fs::read_to_string(&absolute) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WorkflowPolicy::default());
        }
        Err(error) => {
            return Err(Diagnostic::new(
                ErrorKind::Io,
                format!("cannot read workflow policy: {error}"),
            )
            .at_path(path.as_str()));
        }
    };
    let policy: WorkflowPolicy = serde_json::from_str(&source).map_err(|error| {
        Diagnostic::new(
            ErrorKind::InvalidConfiguration,
            format!("cannot parse workflow policy JSON: {error}"),
        )
        .at_path(path.as_str())
    })?;
    if policy.schema_version != 1 {
        return Err(Diagnostic::new(
            ErrorKind::InvalidConfiguration,
            "workflow schema_version must be 1",
        )
        .at_path(path.as_str()));
    }
    Ok(policy)
}
