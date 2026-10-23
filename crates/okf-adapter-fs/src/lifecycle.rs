use std::{
    fs,
    path::{Path, PathBuf},
};

use okf_application::lifecycle::{InstallReport, LifecycleFs, ValidationReport};
use okf_domain::{
    diagnostics::{Diagnostic, ErrorKind},
    paths::RepoPath,
};
use walkdir::WalkDir;

#[derive(Default)]
pub struct FsLifecycle;

impl LifecycleFs for FsLifecycle {
    fn initialize_files(
        &self,
        root: &Path,
        profiles: &[String],
    ) -> Result<Vec<String>, Diagnostic> {
        crate::templates::initialize(root, profiles)
    }

    fn replace_generated(&self, root: &Path, relative: &str, body: &str) -> Result<(), Diagnostic> {
        let root =
            fs::canonicalize(root).map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()))?;
        let path = RepoPath::new(&root, Path::new(relative))?;
        let target = root.join(path.as_path());
        let current = if target.exists() {
            fs::read_to_string(&target)
                .map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()).at_path(relative))?
        } else {
            let title = target
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .replace('-', " ")
                .split_whitespace()
                .map(capitalize)
                .collect::<Vec<_>>()
                .join(" ");
            let text = format!(
                "# {title}\n\n{}{}\n",
                crate::generated_blocks::GENERATED_START,
                crate::generated_blocks::GENERATED_END
            );
            write(&root, relative, &text)?;
            text
        };
        let replacement = crate::generated_blocks::replace_generated(&current, body);
        if replacement != current {
            write(&root, relative, &replacement)?
        }
        Ok(())
    }

    fn install_adapters(
        &self,
        root: &Path,
        git_dir: Option<&Path>,
        workflow_profiles: bool,
    ) -> Result<InstallReport, Diagnostic> {
        let report = crate::instructions::install(root, git_dir, workflow_profiles)?;
        Ok(InstallReport {
            installed: report.installed,
            conflicts: report.conflicts,
        })
    }

    fn validate_files(&self, root: &Path) -> Result<ValidationReport, Diagnostic> {
        let root =
            fs::canonicalize(root).map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()))?;
        let mut report = ValidationReport {
            ok: true,
            ..Default::default()
        };
        for (relative, _) in crate::templates::REQUIRED_FILES {
            if !root.join(".okf").join(relative).is_file() {
                report
                    .errors
                    .push(format!("Missing required knowledge file: .okf/{relative}"));
            }
        }
        let manifest = root.join(".okf/manifest.yaml");
        if manifest.is_file() {
            let text = fs::read_to_string(&manifest).unwrap_or_default();
            let versions = text
                .lines()
                .filter_map(|line| line.strip_prefix("schema_version:").map(str::trim))
                .collect::<Vec<_>>();
            let names = text
                .lines()
                .filter_map(|line| line.strip_prefix("name:").map(str::trim))
                .collect::<Vec<_>>();
            if versions != ["1"] || names.len() != 1 || names[0].is_empty() {
                report.errors.push(
                    "manifest.yaml must contain one schema_version: 1 and one non-empty name"
                        .to_owned(),
                );
            }
        }
        let config = root.join(".okf/workflows/config.json");
        if config.exists() {
            match crate::load_workflow_policy(&root) {
                Err(error) => report.errors.push(error.to_string()),
                Ok(policy) => {
                    let mut expected = vec!["active.md".to_owned(), "classification.md".to_owned()];
                    for method in policy.enabled_profiles {
                        expected.push(format!("profiles/{}.md", method_name(method)));
                        if method == okf_domain::workflow::WorkflowMethod::Gtdd {
                            expected.extend([
                                "roles/coder.md".to_owned(),
                                "roles/tester.md".to_owned(),
                                "roles/auditor.md".to_owned(),
                            ]);
                        }
                    }
                    for relative in expected {
                        let target = root.join(".okf/workflows").join(&relative);
                        if !target.is_file()
                            || fs::symlink_metadata(&target)
                                .is_ok_and(|m| m.file_type().is_symlink())
                        {
                            report.errors.push(format!("Workflow policy references missing or unsafe file: .okf/workflows/{relative}"));
                        }
                    }
                }
            }
        }
        let bundle = root.join(".okf");
        if bundle.is_dir() {
            for entry in WalkDir::new(&bundle).follow_links(false) {
                let entry = entry.map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()))?;
                if !entry.file_type().is_file()
                    || entry.path().extension().and_then(|v| v.to_str()) != Some("md")
                {
                    continue;
                }
                let relative = entry
                    .path()
                    .strip_prefix(&root)
                    .unwrap_or(entry.path())
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = fs::read_to_string(entry.path()).unwrap_or_default();
                if let Err(error) = crate::generated_blocks::validate_generated(&text, &relative) {
                    report.errors.push(error.to_string());
                }
                report
                    .errors
                    .extend(validate_links(&root, entry.path(), &text));
            }
        }
        report.ok = report.errors.is_empty();
        Ok(report)
    }

    fn record_decision(
        &self,
        root: &Path,
        title: &str,
        rationale: &str,
        sources: &[String],
    ) -> Result<String, Diagnostic> {
        let root =
            fs::canonicalize(root).map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()))?;
        let slug = slug(title);
        let timestamp = utc_stamp(std::time::SystemTime::now());
        let relative = format!(".okf/decisions/{timestamp}-{slug}.md");
        let refs = if sources.is_empty() {
            "- None recorded".to_owned()
        } else {
            sources
                .iter()
                .map(|source| format!("- `{source}`"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let content =
            format!("# {title}\n\n## Rationale\n{rationale}\n\n## Source references\n{refs}\n");
        write(&root, &relative, &content)?;
        Ok(relative)
    }

    fn install_codex_skill(&self, skills_dir: Option<&Path>) -> Result<String, Diagnostic> {
        let base = if let Some(path) = skills_dir {
            path.to_path_buf()
        } else if let Some(path) = std::env::var_os("CODEX_HOME") {
            PathBuf::from(path).join("skills")
        } else if let Some(path) = std::env::var_os("USERPROFILE") {
            PathBuf::from(path).join(".codex/skills")
        } else if let Some(path) = std::env::var_os("HOME") {
            PathBuf::from(path).join(".codex/skills")
        } else {
            return Err(Diagnostic::new(
                ErrorKind::Io,
                "cannot resolve Codex skills directory; pass --skills-dir",
            ));
        };
        let target = base.join(crate::templates::CODEX_SKILL_NAME);
        fs::create_dir_all(&target).map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()))?;
        fs::write(
            target.join("SKILL.md"),
            crate::templates::CODEX_SKILL_CONTENT,
        )
        .map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()))?;
        Ok(target.to_string_lossy().into_owned())
    }
}

fn method_name(method: okf_domain::workflow::WorkflowMethod) -> &'static str {
    match method {
        okf_domain::workflow::WorkflowMethod::Sdd => "sdd",
        okf_domain::workflow::WorkflowMethod::Tdd => "tdd",
        okf_domain::workflow::WorkflowMethod::Gtdd => "gtdd",
    }
}
fn write(root: &Path, relative: &str, content: &str) -> Result<(), Diagnostic> {
    let path = RepoPath::new(root, Path::new(relative))?;
    let target = root.join(path.as_path());
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()).at_path(relative))?;
    }
    fs::write(target, content)
        .map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()).at_path(relative))
}
fn validate_links(root: &Path, document: &Path, text: &str) -> Vec<String> {
    let mut errors = Vec::new();
    for target in crate::markdown_links::markdown_targets(text, false) {
        if target.starts_with('#')
            || target.contains("://")
            || target.split_once(':').is_some_and(|(scheme, _)| {
                !scheme.is_empty()
                    && scheme
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
            })
        {
            continue;
        }
        let path = target.split(['#', '?']).next().unwrap_or("");
        if path.is_empty() {
            continue;
        }
        let decoded = decode_percent(&crate::markdown_links::unescape(path));
        let candidate = document.parent().unwrap_or(root).join(decoded);
        if !resolves_inside(root, &candidate) {
            errors.push(format!(
                "Markdown link escapes repository: {} -> {target}",
                document.strip_prefix(root).unwrap_or(document).display()
            ));
        } else if !candidate.exists() {
            errors.push(format!(
                "Broken Markdown link: {} -> {target}",
                document.strip_prefix(root).unwrap_or(document).display()
            ));
        }
    }
    errors
}

fn resolves_inside(root: &Path, candidate: &Path) -> bool {
    let existing = candidate
        .ancestors()
        .find(|path| fs::symlink_metadata(path).is_ok());
    let Some(existing) = existing else {
        return false;
    };
    let Ok(root) = fs::canonicalize(root) else {
        return false;
    };
    let Ok(ancestor) = fs::canonicalize(existing) else {
        return false;
    };
    ancestor.starts_with(root)
}
fn decode_percent(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut result = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(a), Some(b)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            )
        {
            result.push((a * 16 + b) as u8);
            i += 3;
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&result).into_owned()
}
fn slug(title: &str) -> String {
    let mut slug = String::new();
    for character in title.to_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character)
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-')
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "decision".to_owned()
    } else {
        slug.to_owned()
    }
}
fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}
fn utc_stamp(time: std::time::SystemTime) -> String {
    let seconds = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = seconds.div_euclid(86400);
    let hour = seconds.rem_euclid(86400) / 3600;
    let minute = seconds.rem_euclid(3600) / 60;
    let second = seconds.rem_euclid(60);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}")
}
