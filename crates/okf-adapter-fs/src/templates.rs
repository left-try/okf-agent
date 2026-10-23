pub const REQUIRED_FILES: &[(&str, &str)] = &[
    ("manifest.yaml", "schema_version: 1\nname: {name}\n"),
    (
        "index.md",
        "# Repository Knowledge\n\n<!-- okf:generated:start -->\n<!-- okf:generated:end -->\n",
    ),
    (
        "application/overview.md",
        "# Application Overview\n\n<!-- okf:generated:start -->\n<!-- okf:generated:end -->\n\n## Curated intent\nDescribe the product purpose here.\n",
    ),
    (
        "architecture/index.md",
        "# Architecture\n\n<!-- okf:generated:start -->\n<!-- okf:generated:end -->\n\n## Curated constraints\nRecord architectural constraints here.\n",
    ),
    (
        "operations/development.md",
        "# Development\n\n<!-- okf:generated:start -->\n<!-- okf:generated:end -->\n",
    ),
];

pub const CODEX_SKILL_NAME: &str = "okf-repository-brain";
pub const CODEX_SKILL_CONTENT: &str = "---\nname: okf-repository-brain\ndescription: Automatically initialize and maintain the OKF Repository Brain for every repository-based coding task. Use whenever inspecting, planning, changing, testing, reviewing, or debugging code in a local repository, even when the user does not mention OKF.\n---\n\nRun the OKF lifecycle automatically for every code repository task.\n\nAt the start of work, resolve the repository root. Run `okf-agent init <root>` when `.okf/` is absent, then run `okf-agent index <root>` when the local index is absent or stale. Read `AGENTS.md` and retrieve OKF context before planning or editing.\n\nAfter source changes and before the final response, run `okf-agent update <root> --changed-files <changed paths>` followed by `okf-agent validate <root>`. Fix validation failures caused by the current work. Preserve curated content outside generated OKF blocks.\n\nDo this without asking the user to invoke OKF. If the command is unavailable, state that the OKF installation is incomplete; do not silently skip the lifecycle.\n";

pub fn install_profiles(
    root: &std::path::Path,
    profiles: &[String],
) -> Result<okf_domain::workflow::ProfileInstallResult, okf_domain::diagnostics::Diagnostic> {
    use okf_domain::diagnostics::{Diagnostic, ErrorKind};
    use serde_json::Value;
    let allowed = ["sdd", "tdd", "gtdd"];
    let mut selected = Vec::<&str>::new();
    for profile in profiles.iter().map(String::as_str) {
        if !selected.contains(&profile) {
            selected.push(profile)
        }
    }
    let invalid = selected
        .iter()
        .filter(|value| !allowed.contains(value))
        .copied()
        .collect::<Vec<_>>();
    if !invalid.is_empty() {
        return Err(Diagnostic::new(
            ErrorKind::InvalidConfiguration,
            format!("Unknown workflow profile(s): {}", invalid.join(", ")),
        ));
    }
    if selected.is_empty() {
        return Err(Diagnostic::new(
            ErrorKind::InvalidConfiguration,
            "Select at least one workflow profile",
        ));
    }
    let root =
        std::fs::canonicalize(root).map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()))?;
    let mut result = okf_domain::workflow::ProfileInstallResult::default();
    let workflow_dir = root.join(".okf/workflows");
    let config_relative = ".okf/workflows/config.json";
    let config_path = crate::paths::RepoPath::new(&root, std::path::Path::new(config_relative))?;
    let config = root.join(config_path.as_path());
    if std::fs::symlink_metadata(&config).is_ok_and(|m| m.file_type().is_symlink()) {
        result
            .conflicts
            .push(format!("{config_relative}: unsafe symlink preserved"));
        return Ok(result);
    }
    let current = if config.exists() {
        match std::fs::read_to_string(&config)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        {
            Some(value)
                if serde_json::from_value::<okf_domain::workflow::WorkflowPolicy>(
                    value.clone(),
                )
                .is_ok_and(|policy| policy.schema_version == 1) =>
            {
                Some(value)
            }
            _ => {
                result.conflicts.push(format!(
                    "{config_relative}: preserved existing invalid policy"
                ));
                None
            }
        }
    } else {
        None
    };
    if config.exists() && current.is_none() { /* Keep the invalid user file untouched. */
    } else {
        let mut value = current.unwrap_or_else(|| {
            serde_json::from_str(include_str!(
                "../../../okf_agent/templates/workflows/config.json"
            ))
            .expect("packaged workflow config is valid")
        });
        let requested = Value::Array(
            selected
                .iter()
                .map(|name| Value::String((*name).to_owned()))
                .collect(),
        );
        if config.exists() {
            if value.get("enabled_profiles") != Some(&requested) {
                value["enabled_profiles"] = requested;
                let text = serde_json::to_string_pretty(&value)
                    .map_err(|e| Diagnostic::new(ErrorKind::Internal, e.to_string()))?
                    + "\n";
                write_repo(&root, config_relative, &text)?;
                result.updated.push(config_relative.into())
            } else {
                result.retained.push(config_relative.into())
            }
        } else {
            value["enabled_profiles"] = requested;
            let text = serde_json::to_string_pretty(&value)
                .map_err(|e| Diagnostic::new(ErrorKind::Internal, e.to_string()))?
                + "\n";
            write_repo(&root, config_relative, &text)?;
            result.created.push(config_relative.into())
        }
    }
    let mut relative_files = vec!["active.md", "classification.md"];
    for name in &selected {
        relative_files.push(match *name {
            "sdd" => "profiles/sdd.md",
            "tdd" => "profiles/tdd.md",
            _ => "profiles/gtdd.md",
        });
    }
    if selected.contains(&"gtdd") {
        relative_files.extend(["roles/coder.md", "roles/tester.md", "roles/auditor.md"])
    }
    for relative in relative_files {
        let full = format!(".okf/workflows/{relative}");
        let repo_path = crate::paths::RepoPath::new(&root, std::path::Path::new(&full))?;
        let target = root.join(repo_path.as_path());
        if target.exists() {
            result.retained.push(full);
            continue;
        }
        let content = workflow_template(relative).ok_or_else(|| {
            Diagnostic::new(ErrorKind::NotFound, "packaged workflow template is missing")
                .at_path(&full)
        })?;
        write_repo(&root, &full, content)?;
        result.created.push(full);
    }
    let _ = workflow_dir;
    Ok(result)
}

fn workflow_template(relative: &str) -> Option<&'static str> {
    Some(match relative {
        "active.md" => include_str!("../../../okf_agent/templates/workflows/active.md"),
        "classification.md" => {
            include_str!("../../../okf_agent/templates/workflows/classification.md")
        }
        "profiles/sdd.md" => include_str!("../../../okf_agent/templates/workflows/profiles/sdd.md"),
        "profiles/tdd.md" => include_str!("../../../okf_agent/templates/workflows/profiles/tdd.md"),
        "profiles/gtdd.md" => {
            include_str!("../../../okf_agent/templates/workflows/profiles/gtdd.md")
        }
        "roles/coder.md" => include_str!("../../../okf_agent/templates/workflows/roles/coder.md"),
        "roles/tester.md" => include_str!("../../../okf_agent/templates/workflows/roles/tester.md"),
        "roles/auditor.md" => {
            include_str!("../../../okf_agent/templates/workflows/roles/auditor.md")
        }
        _ => return None,
    })
}

fn write_repo(
    root: &std::path::Path,
    relative: &str,
    content: &str,
) -> Result<(), okf_domain::diagnostics::Diagnostic> {
    let path = crate::paths::RepoPath::new(root, std::path::Path::new(relative))?;
    let target = root.join(path.as_path());
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            okf_domain::diagnostics::Diagnostic::new(
                okf_domain::diagnostics::ErrorKind::Io,
                error.to_string(),
            )
            .at_path(relative)
        })?;
    }
    std::fs::write(target, content).map_err(|error| {
        okf_domain::diagnostics::Diagnostic::new(
            okf_domain::diagnostics::ErrorKind::Io,
            error.to_string(),
        )
        .at_path(relative)
    })
}

pub fn initialize(
    root: &std::path::Path,
    profiles: &[String],
) -> Result<Vec<String>, okf_domain::diagnostics::Diagnostic> {
    let root = std::fs::canonicalize(root).map_err(|error| {
        okf_domain::diagnostics::Diagnostic::new(
            okf_domain::diagnostics::ErrorKind::Io,
            error.to_string(),
        )
    })?;
    crate::paths::RepoPath::new(&root, std::path::Path::new(".okf-index"))?;
    let mut created = Vec::new();
    for (relative, template) in REQUIRED_FILES {
        let path =
            crate::paths::RepoPath::new(&root, std::path::Path::new(&format!(".okf/{relative}")))?;
        let target = root.join(path.as_path());
        if !target.exists() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    okf_domain::diagnostics::Diagnostic::new(
                        okf_domain::diagnostics::ErrorKind::Io,
                        error.to_string(),
                    )
                })?;
            }
            std::fs::write(
                &target,
                template.replace(
                    "{name}",
                    &root.file_name().unwrap_or_default().to_string_lossy(),
                ),
            )
            .map_err(|error| {
                okf_domain::diagnostics::Diagnostic::new(
                    okf_domain::diagnostics::ErrorKind::Io,
                    error.to_string(),
                )
                .at_path(path.as_str())
            })?;
            created.push(path.as_str().to_owned());
        }
    }
    if !profiles.is_empty() {
        let installed = install_profiles(&root, profiles)?;
        created.extend(installed.created);
        created.extend(installed.updated);
    }
    for relative in [".okf/decisions", ".okf/changes", ".okf-index"] {
        let safe_path = crate::paths::RepoPath::new(&root, std::path::Path::new(relative))?;
        std::fs::create_dir_all(root.join(safe_path.as_path())).map_err(|error| {
            okf_domain::diagnostics::Diagnostic::new(
                okf_domain::diagnostics::ErrorKind::Io,
                error.to_string(),
            )
            .at_path(relative)
        })?;
    }
    ignore_local_index(&root)?;
    Ok(created)
}

fn ignore_local_index(root: &std::path::Path) -> Result<(), okf_domain::diagnostics::Diagnostic> {
    use okf_domain::diagnostics::{Diagnostic, ErrorKind};
    use std::path::Path;

    let relative = ".gitignore";
    let safe_path = crate::paths::RepoPath::new(root, Path::new(relative))?;
    let path = root.join(safe_path.as_path());
    let content = if path.is_file() {
        std::fs::read_to_string(&path)
            .map_err(|error| Diagnostic::new(ErrorKind::Io, error.to_string()).at_path(relative))?
    } else if path.exists() {
        return Err(
            Diagnostic::new(ErrorKind::InvalidPath, ".gitignore must be a regular file")
                .at_path(relative),
        );
    } else {
        String::new()
    };

    let exact_rules = [".okf-index", ".okf-index/", "/.okf-index", "/.okf-index/"];
    if content
        .lines()
        .any(|line| exact_rules.contains(&line.trim()))
    {
        return Ok(());
    }

    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let separator = if content.is_empty() || content.ends_with(['\n', '\r']) {
        ""
    } else {
        newline
    };
    let updated = format!("{content}{separator}.okf-index/{newline}");
    std::fs::write(&path, updated)
        .map_err(|error| Diagnostic::new(ErrorKind::Io, error.to_string()).at_path(relative))
}
