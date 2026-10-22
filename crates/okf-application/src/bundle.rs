use std::{collections::BTreeMap, path::Path};

use okf_domain::diagnostics::Diagnostic;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const PROFILE_CONTRACT: &str = "okf-agent.bundle";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawPage {
    pub path: String,
    pub metadata: Value,
    pub body: String,
    pub digest: String,
    pub parse_error: Option<String>,
    pub targets: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleDiagnostic {
    pub path: Option<String>,
    pub severity: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BundlePage {
    pub path: String,
    pub title: String,
    pub doc_type: String,
    pub aliases: Vec<String>,
    pub frontmatter: Value,
    pub body: String,
    pub content_hash: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BundleLink {
    pub source: String,
    pub target: Option<String>,
    pub locator: String,
    pub relation: Option<String>,
    pub status: String,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BundleProvenance {
    pub document: String,
    pub source: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedBundle {
    pub pages: Vec<BundlePage>,
    pub links: Vec<BundleLink>,
    pub provenance: Vec<BundleProvenance>,
    pub diagnostics: Vec<BundleDiagnostic>,
    pub conformant: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleIndexReport {
    pub profile_contract: Value,
    pub root: String,
    pub index_root: String,
    pub documents: usize,
    pub conformant: bool,
    pub diagnostics: Vec<BundleDiagnostic>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleSearchHit {
    pub path: String,
    pub excerpt: String,
    pub title: String,
    #[serde(rename = "type")]
    pub doc_type: String,
}

pub trait BundleSource {
    fn read_all(&self, root: &Path, wikilinks: bool) -> Result<Vec<RawPage>, Diagnostic>;
}
pub trait BundleStore {
    fn rebuild(
        &self,
        index_root: &Path,
        root_label: &str,
        prepared: &PreparedBundle,
    ) -> Result<BundleIndexReport, Diagnostic>;
    fn search(
        &self,
        index_root: &Path,
        query: &str,
        limit: usize,
    ) -> Result<Vec<BundleSearchHit>, Diagnostic>;
    fn graph(
        &self,
        index_root: &Path,
        path: &str,
        depth: usize,
        backlinks: bool,
    ) -> Result<Value, Diagnostic>;
    fn export(&self, index_root: &Path) -> Result<Value, Diagnostic>;
}

pub fn index(
    root: &Path,
    index_root: &Path,
    wikilinks: bool,
    source: &impl BundleSource,
    store: &impl BundleStore,
) -> Result<BundleIndexReport, Diagnostic> {
    let pages = source.read_all(root, wikilinks)?;
    let prepared = prepare(pages);
    store.rebuild(index_root, &root.to_string_lossy(), &prepared)
}

pub fn search(
    index_root: &Path,
    query: &str,
    limit: usize,
    store: &impl BundleStore,
) -> Result<Vec<BundleSearchHit>, Diagnostic> {
    store.search(index_root, query, limit)
}
pub fn graph(
    index_root: &Path,
    path: &str,
    depth: usize,
    backlinks: bool,
    store: &impl BundleStore,
) -> Result<Value, Diagnostic> {
    store.graph(index_root, path, depth, backlinks)
}
pub fn export(index_root: &Path, store: &impl BundleStore) -> Result<Value, Diagnostic> {
    store.export(index_root)
}

pub fn prepare(raw_pages: Vec<RawPage>) -> PreparedBundle {
    let known = raw_pages
        .iter()
        .map(|page| page.path.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut basenames = raw_pages.iter().fold(
        BTreeMap::<String, Vec<String>>::new(),
        |mut result, page| {
            result
                .entry(
                    page.path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&page.path)
                        .to_lowercase(),
                )
                .or_default()
                .push(page.path.clone());
            result
        },
    );
    for values in basenames.values_mut() {
        values.sort();
    }
    let mut pages = Vec::new();
    let mut links = Vec::new();
    let mut provenance = Vec::new();
    let mut diagnostics = Vec::new();
    for page in raw_pages {
        let metadata = page.metadata.as_object().cloned().unwrap_or_default();
        let title = metadata
            .get("title")
            .filter(|v| !v.is_null())
            .map(value_string)
            .unwrap_or_default();
        let doc_type = metadata
            .get("type")
            .filter(|v| !v.is_null())
            .map(value_string)
            .unwrap_or_default();
        let aliases = match metadata.get("aliases") {
            Some(Value::String(value)) => vec![value.clone()],
            Some(Value::Array(values)) => values.iter().map(value_string).collect(),
            _ => Vec::new(),
        };
        pages.push(BundlePage {
            path: page.path.clone(),
            title,
            doc_type,
            aliases,
            frontmatter: page.metadata.clone(),
            body: page.body.clone(),
            content_hash: page.digest.clone(),
        });
        let reserved_index = page.path.rsplit('/').next() == Some("index.md");
        let reserved_log = page.path.rsplit('/').next() == Some("log.md");
        if reserved_index {
            if page.parse_error.is_none()
                && !metadata.is_empty()
                && (page.path != "index.md" || metadata.keys().any(|key| key != "okf_version"))
            {
                diagnostic(
                    &mut diagnostics,
                    &page.path,
                    "error",
                    "index.md frontmatter is only allowed at the bundle root and may contain only okf_version",
                );
            }
            if let Some(error) = &page.parse_error
                && !error.starts_with("Missing YAML frontmatter")
            {
                diagnostic(&mut diagnostics, &page.path, "error", error);
            }
            if page.path == "index.md"
                && metadata
                    .get("okf_version")
                    .is_some_and(|value| value != "0.2")
            {
                diagnostic(
                    &mut diagnostics,
                    &page.path,
                    "warning",
                    "Unrecognized okf_version; consumed best-effort",
                );
            }
            if !page.body.lines().any(|line| {
                let trimmed = line.trim_start();
                let count = trimmed.chars().take_while(|c| *c == '#').count();
                (1..=6).contains(&count)
                    && trimmed[count..].starts_with(char::is_whitespace)
                    && !trimmed[count..].trim().is_empty()
            }) {
                diagnostic(
                    &mut diagnostics,
                    &page.path,
                    "error",
                    "index.md must contain at least one section heading",
                );
            }
        } else if reserved_log {
            if !metadata.is_empty() {
                diagnostic(
                    &mut diagnostics,
                    &page.path,
                    "error",
                    "log.md must not contain frontmatter",
                );
            }
            if let Some(error) = &page.parse_error
                && !error.starts_with("Missing YAML frontmatter")
            {
                diagnostic(&mut diagnostics, &page.path, "error", error);
            }
            let dates = page
                .body
                .lines()
                .filter_map(|line| {
                    line.strip_prefix("## ")
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned)
                })
                .collect::<Vec<_>>();
            if dates.iter().any(|value| !valid_date_heading(value)) {
                diagnostic(
                    &mut diagnostics,
                    &page.path,
                    "error",
                    "log.md date headings must use YYYY-MM-DD and valid calendar dates",
                );
            } else if dates.windows(2).any(|pair| pair[0] < pair[1]) {
                diagnostic(
                    &mut diagnostics,
                    &page.path,
                    "error",
                    "log.md date headings must be newest first",
                );
            } else if dates.is_empty() {
                diagnostic(
                    &mut diagnostics,
                    &page.path,
                    "error",
                    "log.md must contain at least one date heading",
                );
            }
        } else {
            if let Some(error) = &page.parse_error {
                diagnostic(&mut diagnostics, &page.path, "error", error);
            } else if metadata
                .get("type")
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                diagnostic(
                    &mut diagnostics,
                    &page.path,
                    "error",
                    "Missing non-empty string type",
                );
            }
        }
        let sources = metadata.get("sources").map(as_list).unwrap_or_default();
        for source in sources {
            provenance.push(BundleProvenance {
                document: page.path.clone(),
                source: source.clone(),
            });
            if let Some(locator) = source
                .get("resource")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
            {
                let (target, status, _) = resolve_target(&page.path, locator, &known, &basenames);
                links.push(BundleLink {
                    source: target.clone().unwrap_or_else(|| locator.to_owned()),
                    target: Some(page.path.clone()),
                    locator: locator.to_owned(),
                    relation: Some("provenance".to_owned()),
                    status: if target.is_some() {
                        status
                    } else {
                        "external".to_owned()
                    },
                    diagnostic: None,
                });
            }
        }
        for locator in page.targets {
            let (target, status, error) = resolve_target(&page.path, &locator, &known, &basenames);
            let relation = relation_for(&metadata, &locator, target.as_deref());
            if let Some(message) = &error {
                diagnostic(&mut diagnostics, &page.path, "diagnostic", message);
            }
            links.push(BundleLink {
                source: page.path.clone(),
                target,
                locator,
                relation,
                status,
                diagnostic: error,
            });
        }
    }
    let conformant = !diagnostics.iter().any(|item| item.severity == "error");
    PreparedBundle {
        pages,
        links,
        provenance,
        diagnostics,
        conformant,
    }
}

fn value_string(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}
fn as_list(value: &Value) -> Vec<&Value> {
    match value {
        Value::Array(values) => values.iter().collect(),
        Value::Object(_) | Value::String(_) => vec![value],
        _ => Vec::new(),
    }
}
fn diagnostic(out: &mut Vec<BundleDiagnostic>, path: &str, severity: &str, message: &str) {
    out.push(BundleDiagnostic {
        path: Some(path.to_owned()),
        severity: severity.to_owned(),
        message: message.to_owned(),
    });
}

fn resolve_target(
    source: &str,
    raw: &str,
    known: &std::collections::BTreeSet<String>,
    basenames: &BTreeMap<String, Vec<String>>,
) -> (Option<String>, String, Option<String>) {
    let (without_fragment, _) = raw.split_once('#').unwrap_or((raw, ""));
    let path_part = without_fragment
        .split_once('?')
        .map_or(without_fragment, |(path, _)| path);
    let has_scheme = path_part.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme.chars().enumerate().all(|(index, ch)| {
                ch.is_ascii_alphabetic()
                    || (index > 0 && (ch.is_ascii_digit() || matches!(ch, '+' | '-' | '.')))
            })
    });
    if has_scheme || path_part.starts_with("//") || raw.starts_with('#') {
        return (None, "external".to_owned(), None);
    }
    let decoded = percent_decode(path_part).replace('\\', "/");
    if decoded.is_empty() {
        return (Some(source.to_owned()), "resolved".to_owned(), None);
    }
    let mut parts = Vec::new();
    let combined = if decoded.starts_with('/') {
        decoded.trim_start_matches('/').to_owned()
    } else {
        format!(
            "{}/{}",
            source.rsplit_once('/').map_or("", |(parent, _)| parent),
            decoded
        )
    };
    for part in combined.split('/') {
        match part {
            "" | "." => (),
            ".." if parts.pop().is_none() => {
                return (
                    None,
                    "broken".to_owned(),
                    Some(format!("Link escapes bundle root: {raw}")),
                );
            }
            ".." => (),
            item => parts.push(item.to_owned()),
        }
    }
    let normalized = parts.join("/");
    let mut candidates = vec![normalized.clone()];
    if !normalized
        .rsplit('/')
        .next()
        .unwrap_or(&normalized)
        .contains('.')
    {
        candidates.extend([format!("{normalized}.md"), format!("{normalized}/index.md")]);
    }
    if let Some(candidate) = candidates
        .iter()
        .find(|candidate| known.contains(candidate.as_str()))
    {
        return (Some(candidate.clone()), "resolved".to_owned(), None);
    }
    let basename = normalized
        .rsplit('/')
        .next()
        .unwrap_or(&normalized)
        .to_lowercase();
    if let Some(matches) = basenames.get(&basename) {
        return (
            None,
            "ambiguous".to_owned(),
            Some(format!(
                "Ambiguous link {raw:?}; matching files: {}",
                matches.join(", ")
            )),
        );
    }
    (
        None,
        "broken".to_owned(),
        Some(format!("Broken link: {raw}")),
    )
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) = (
                (bytes[index + 1] as char).to_digit(16),
                (bytes[index + 2] as char).to_digit(16),
            )
        {
            decoded.push((high * 16 + low) as u8);
            index += 3;
            continue;
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}
fn relation_for(
    metadata: &serde_json::Map<String, Value>,
    locator: &str,
    target: Option<&str>,
) -> Option<String> {
    let relations = metadata.get("relations").map(as_list).unwrap_or_default();
    relations
        .into_iter()
        .find(|item| {
            let candidate = item
                .get("target")
                .or_else(|| item.get("path"))
                .or_else(|| item.get("resource"));
            candidate.is_some_and(|v| v == locator || target.is_some_and(|resolved| v == resolved))
        })
        .and_then(|item| {
            item.get("relation")
                .or_else(|| item.get("type"))
                .or_else(|| item.get("kind"))
        })
        .map(value_string)
}
fn valid_date_heading(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        parts[0].parse::<u32>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<u32>(),
    ) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    day > 0 && day <= days
}

pub fn validation_result(report: &BundleIndexReport) -> Value {
    let errors = report
        .diagnostics
        .iter()
        .filter(|item| item.severity == "error")
        .cloned()
        .collect::<Vec<_>>();
    let warnings = report
        .diagnostics
        .iter()
        .filter(|item| item.severity != "error")
        .cloned()
        .collect::<Vec<_>>();
    json!({"ok": report.conformant, "conformant": report.conformant, "documents": report.documents, "diagnostics": report.diagnostics, "errors": errors, "warnings": warnings})
}

pub fn require_index(index_root: &Path) -> Result<(), Diagnostic> {
    if index_root.join("knowledge.sqlite3").is_file() {
        Ok(())
    } else {
        Err(Diagnostic::new(
            okf_domain::diagnostics::ErrorKind::NotFound,
            "Bundle index not found; run bundle index first",
        )
        .at_path(index_root.to_string_lossy().to_string()))
    }
}
