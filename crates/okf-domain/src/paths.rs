use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::diagnostics::{Diagnostic, ErrorKind};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RepoPath {
    relative: PathBuf,
    display: String,
}

impl RepoPath {
    pub fn new(root: &Path, input: &Path) -> Result<Self, Diagnostic> {
        if input.as_os_str().is_empty() {
            return Err(Diagnostic::new(
                ErrorKind::InvalidPath,
                "repository path is empty",
            ));
        }

        let root = fs::canonicalize(root).map_err(|error| {
            Diagnostic::new(
                ErrorKind::Io,
                format!("cannot resolve repository root: {error}"),
            )
        })?;
        let relative = if input.is_absolute() {
            let existing_ancestor = input
                .ancestors()
                .find(|ancestor| fs::symlink_metadata(ancestor).is_ok())
                .ok_or_else(|| {
                    Diagnostic::new(
                        ErrorKind::InvalidPath,
                        "absolute path has no existing ancestor",
                    )
                })?;
            let canonical_ancestor = fs::canonicalize(existing_ancestor).map_err(|error| {
                Diagnostic::new(
                    ErrorKind::InvalidPath,
                    format!("cannot safely resolve absolute path: {error}"),
                )
            })?;
            let root_relative = canonical_ancestor.strip_prefix(&root).map_err(|_| {
                Diagnostic::new(
                    ErrorKind::InvalidPath,
                    "absolute path is outside the repository root",
                )
            })?;
            let remaining = input.strip_prefix(existing_ancestor).map_err(|_| {
                Diagnostic::new(ErrorKind::InvalidPath, "cannot normalize absolute path")
            })?;
            root_relative.join(remaining)
        } else {
            input.to_path_buf()
        };

        let mut normalized = PathBuf::new();
        for component in relative.components() {
            match component {
                Component::Normal(value) => normalized.push(value),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(Diagnostic::new(
                        ErrorKind::InvalidPath,
                        "repository path must not contain parent or root components",
                    ));
                }
            }
        }

        if normalized.as_os_str().is_empty() {
            return Err(Diagnostic::new(
                ErrorKind::InvalidPath,
                "repository path must name a file or directory below the root",
            ));
        }

        let candidate = root.join(&normalized);
        let existing_ancestor = candidate
            .ancestors()
            .find(|ancestor| fs::symlink_metadata(ancestor).is_ok())
            .ok_or_else(|| {
                Diagnostic::new(
                    ErrorKind::InvalidPath,
                    "repository path has no existing ancestor",
                )
            })?;
        let canonical_ancestor = fs::canonicalize(existing_ancestor).map_err(|error| {
            Diagnostic::new(
                ErrorKind::InvalidPath,
                format!("cannot safely resolve repository path ancestor: {error}"),
            )
        })?;
        if !canonical_ancestor.starts_with(&root) {
            return Err(Diagnostic::new(
                ErrorKind::InvalidPath,
                "repository path resolves outside the repository root",
            ));
        }

        let display = normalized.to_string_lossy().replace('\\', "/");
        Ok(Self {
            relative: normalized,
            display,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.display
    }

    pub fn as_path(&self) -> &Path {
        &self.relative
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
