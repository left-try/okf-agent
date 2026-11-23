#![deny(warnings)]
//! Git repository discovery adapter.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
};

use okf_domain::diagnostics::{Diagnostic, ErrorKind};

#[derive(Default)]
pub struct Git;

impl okf_application::lifecycle::GitPort for Git {
    fn changed_files(&self, root: &Path) -> Vec<String> {
        changed_files(root)
    }
    fn git_dir(&self, root: &Path) -> Option<PathBuf> {
        git_dir(root).ok()
    }
    fn diff(&self, root: &Path) -> String {
        repo_diff(root)
    }
}

pub fn repo_root(path: &Path) -> Result<PathBuf, Diagnostic> {
    let mut candidate = std::fs::canonicalize(path)
        .map_err(|error| Diagnostic::new(ErrorKind::Io, error.to_string()))?;
    if !candidate.is_dir() {
        candidate.pop();
    }
    for parent in candidate.ancestors() {
        if git_dir(parent).is_ok() {
            return Ok(parent.to_path_buf());
        }
    }
    Ok(candidate)
}

pub fn git_dir(root: &Path) -> Result<PathBuf, Diagnostic> {
    let marker = root.join(".git");
    if marker.is_dir() {
        return std::fs::canonicalize(&marker)
            .map_err(|error| Diagnostic::new(ErrorKind::Io, error.to_string()));
    }
    let text = std::fs::read_to_string(&marker).map_err(|error| {
        Diagnostic::new(
            ErrorKind::NotFound,
            format!("Git metadata is unavailable: {error}"),
        )
    })?;
    let path = text
        .trim()
        .strip_prefix("gitdir:")
        .ok_or_else(|| Diagnostic::new(ErrorKind::InvalidConfiguration, "invalid .git file"))?
        .trim();
    let path = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    };
    std::fs::canonicalize(path).map_err(|error| Diagnostic::new(ErrorKind::Io, error.to_string()))
}

pub fn changed_files(root: &Path) -> Vec<String> {
    let mut changed = BTreeSet::new();
    for args in [
        ["diff", "--name-only", "HEAD", "--"].as_slice(),
        ["diff", "--cached", "--name-only", "--"].as_slice(),
        ["ls-files", "--others", "--exclude-standard", "--"].as_slice(),
    ] {
        if let Ok(output) = Command::new("git").args(args).current_dir(root).output()
            && output.status.success()
        {
            for line in String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| !line.is_empty())
            {
                changed.insert(line.replace('\\', "/"));
            }
        }
    }
    changed.into_iter().collect()
}

pub fn repo_diff(root: &Path) -> String {
    let mut diff = String::new();
    for args in [
        ["diff", "HEAD", "--"].as_slice(),
        ["diff", "--cached", "--"].as_slice(),
    ] {
        if let Ok(output) = Command::new("git").args(args).current_dir(root).output()
            && output.status.success()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            if !text.is_empty() && !diff.contains(text.trim()) {
                diff.push_str(&text);
                diff.push('\n');
            }
        }
    }
    if let Ok(output) = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard", "--"])
        .current_dir(root)
        .output()
        && output.status.success()
    {
        for line in String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|l| !l.is_empty())
        {
            let file_path = root.join(line);
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                diff.push_str(&format!(
                    "\ndiff --git a/{line} b/{line}\n--- /dev/null\n+++ b/{line}\n@@ -0,0 +1,{} @@\n",
                    content.lines().count()
                ));
                for l in content.lines() {
                    diff.push('+');
                    diff.push_str(l);
                    diff.push('\n');
                }
            }
        }
    }
    diff
}
