use std::{fs, path::Path};

use okf_domain::{
    diagnostics::{Diagnostic, ErrorKind},
    paths::RepoPath,
};

pub const PROTOCOL: &str = "## OKF Repository Protocol\n\nBefore planning or editing, run `okf-agent index .` when the index is absent, then retrieve repository context. After source changes, run `okf-agent update . --changed-files <changed paths>`. Before completion, run `okf-agent validate .`. Preserve curated content outside OKF-generated blocks.\n";
const CURSOR_RULE: &str = "---\ndescription: Maintain OKF repository knowledge before and after every coding task.\nalwaysApply: true\n---\n\n";
const WORKFLOW_START: &str = "<!-- okf:workflow:start -->";
const WORKFLOW_END: &str = "<!-- okf:workflow:end -->";
const WORKFLOW_ROUTING: &str = "<!-- okf:workflow:start -->\n## OKF Workflow Routing\n\nRead `.okf/workflows/active.md` when present. Classify the task and assess risk, then load only the selected workflow profile and assigned role instructions. Use independent GTDD only when separate role contexts and controlled handoffs are available; otherwise report a single-agent adversarial review.\n<!-- okf:workflow:end -->";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InstallReport {
    pub installed: Vec<String>,
    pub conflicts: Vec<String>,
}

pub fn install(
    root: &Path,
    git_dir: Option<&Path>,
    workflow_profiles: bool,
) -> Result<InstallReport, Diagnostic> {
    let root = fs::canonicalize(root).map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()))?;
    let mut report = InstallReport::default();
    let adapters = [
        "AGENTS.md",
        "CLAUDE.md",
        "GEMINI.md",
        ".claude/rules/okf.md",
    ];
    let mut safe = Vec::new();
    for relative in adapters {
        if let Err(error) = ensure_target(&root, relative) {
            report.conflicts.push(format!(
                "{relative}: unsafe path outside repository preserved ({error})"
            ));
            continue;
        }
        let target = root.join(relative);
        if fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()) {
            report
                .conflicts
                .push(format!("{relative}: unsafe symlink preserved"));
            continue;
        }
        if append_protocol(&root, relative)? {
            report.installed.push(relative.to_owned());
        }
        safe.push(relative.to_owned());
    }
    let cursor = ".cursor/rules/okf.mdc";
    if ensure_target(&root, cursor).is_err() {
        report.conflicts.push(format!(
            "{cursor}: unsafe path outside repository preserved"
        ));
    } else if fs::symlink_metadata(root.join(cursor)).is_ok_and(|m| m.file_type().is_symlink()) {
        report
            .conflicts
            .push(format!("{cursor}: unsafe symlink preserved"));
    } else if !root.join(cursor).exists() {
        write_repo(&root, cursor, &format!("{CURSOR_RULE}{PROTOCOL}"))?;
        report.installed.push(cursor.to_owned());
        safe.push(cursor.to_owned());
    } else {
        safe.push(cursor.to_owned());
    }
    if workflow_profiles {
        for relative in safe {
            let current = fs::read_to_string(root.join(&relative)).unwrap_or_default();
            let starts = current.matches(WORKFLOW_START).count();
            let ends = current.matches(WORKFLOW_END).count();
            if starts != ends || starts > 1 {
                report.conflicts.push(format!(
                    "{relative}: malformed or duplicate workflow routing markers"
                ));
                continue;
            }
            let replacement = if starts == 1 {
                let start = current.find(WORKFLOW_START).unwrap();
                let end = current.find(WORKFLOW_END).unwrap() + WORKFLOW_END.len();
                format!(
                    "{}{}{}",
                    &current[..start],
                    WORKFLOW_ROUTING,
                    &current[end..]
                )
            } else {
                format!("{}\n\n{WORKFLOW_ROUTING}\n", current.trim_end())
            };
            if replacement != current {
                write_repo(&root, &relative, &replacement)?;
                report.installed.push(relative);
            }
        }
    }
    if let Some(git_dir) = git_dir {
        install_hooks(&root, git_dir, &mut report)?;
    }
    report.installed.sort();
    report.installed.dedup();
    Ok(report)
}

fn append_protocol(root: &Path, relative: &str) -> Result<bool, Diagnostic> {
    let target = root.join(relative);
    if target.exists() {
        let current = fs::read_to_string(&target).map_err(|e| io_error(relative, e))?;
        if current.contains("## OKF Repository Protocol") {
            return Ok(false);
        }
        write_repo(
            root,
            relative,
            &format!("{}\n\n{PROTOCOL}", current.trim_end()),
        )?;
    } else {
        write_repo(
            root,
            relative,
            &format!("# Repository Instructions\n\n{PROTOCOL}"),
        )?;
    }
    Ok(true)
}
fn install_hooks(
    _root: &Path,
    git_dir: &Path,
    report: &mut InstallReport,
) -> Result<(), Diagnostic> {
    let hooks = git_dir.join("hooks");
    if !hooks.is_dir() {
        return Ok(());
    }
    for (name, command) in [
        ("post-checkout", "okf-agent index . || exit 0\n"),
        ("post-merge", "okf-agent index . || exit 0\n"),
        ("pre-commit", "okf-agent update .\n"),
        ("pre-push", "okf-agent validate .\n"),
    ] {
        let relative = format!(".git/hooks/{name}");
        let target = hooks.join(name);
        if fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()) {
            report
                .conflicts
                .push(format!("{relative}: unsafe symlink preserved"));
            continue;
        }
        if target.exists() {
            let current = fs::read_to_string(&target).unwrap_or_default();
            if !current.contains("# Generated by okf-agent") {
                continue;
            }
        }
        let script = format!("#!/bin/sh\n# Generated by okf-agent\n{command}");
        fs::write(&target, script).map_err(|e| io_error(&relative, e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&target, fs::Permissions::from_mode(0o755));
        }
        report.installed.push(relative);
    }
    Ok(())
}
fn ensure_target(root: &Path, relative: &str) -> Result<RepoPath, Diagnostic> {
    RepoPath::new(root, Path::new(relative))
}
fn write_repo(root: &Path, relative: &str, content: &str) -> Result<(), Diagnostic> {
    let path = ensure_target(root, relative)?;
    let absolute = root.join(path.as_path());
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent).map_err(|e| io_error(relative, e))?;
    }
    fs::write(absolute, content).map_err(|e| io_error(relative, e))
}
fn io_error(path: &str, error: std::io::Error) -> Diagnostic {
    Diagnostic::new(ErrorKind::Io, error.to_string()).at_path(path)
}
