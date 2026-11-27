use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BoundaryRule {
    pub from: String,
    #[serde(default)]
    pub disallow: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DocMapping {
    pub code: String,
    pub doc: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BoundariesConfig {
    #[serde(default)]
    pub rules: Vec<BoundaryRule>,
    #[serde(default)]
    pub boundaries: Vec<BoundaryRule>,
    #[serde(default)]
    pub doc_mappings: Vec<DocMapping>,
}

impl BoundariesConfig {
    pub fn all_rules(&self) -> Vec<BoundaryRule> {
        let mut rules = self.rules.clone();
        rules.extend(self.boundaries.clone());
        rules
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GateReport {
    pub ok: bool,
    pub diff_loc: usize,
    pub loc_budget: usize,
    pub test_weakening: Vec<String>,
    pub escape_hatches: Vec<String>,
    pub boundary_violations: Vec<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn evaluate_diff(
    diff: &str,
    loc_budget: usize,
    allow_weakening: bool,
    allow_escapes: bool,
) -> (usize, Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut diff_loc = 0;
    let mut test_weakening = Vec::new();
    let mut escape_hatches = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let mut current_file: Option<String> = None;
    let mut file_added_tests = 0;
    let mut file_removed_tests = 0;

    let finish_file =
        |file: Option<&str>, added: usize, removed: usize, weakening: &mut Vec<String>| {
            if let Some(f) = file
                && is_test_file(f)
                && removed > added
            {
                weakening.push(format!(
                    "Test weakening in `{f}`: {removed} assertion(s) removed vs {added} added"
                ));
            }
        };

    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            finish_file(
                current_file.as_deref(),
                file_added_tests,
                file_removed_tests,
                &mut test_weakening,
            );
            file_added_tests = 0;
            file_removed_tests = 0;
            current_file = line
                .split_whitespace()
                .nth(2)
                .map(|p| p.trim_start_matches("a/").to_owned());
            continue;
        }

        if line.starts_with("+++ ") {
            if let Some(target) = line.strip_prefix("+++ b/") {
                current_file = Some(target.to_owned());
            }
            continue;
        }
        if line.starts_with("--- ") {
            continue;
        }

        if let Some(content) = line.strip_prefix('+') {
            diff_loc += 1;
            let file_name = current_file.as_deref().unwrap_or("unknown");

            if let Some(hatch) = find_escape_hatch(content) {
                escape_hatches.push(format!(
                    "Escape hatch `{hatch}` in `{file_name}`: `{}`",
                    content.trim()
                ));
            }

            if let Some(f) = current_file.as_deref()
                && is_test_file(f)
            {
                file_added_tests += count_test_indicators(content);
            }
        } else if let Some(content) = line.strip_prefix('-') {
            diff_loc += 1;
            if let Some(f) = current_file.as_deref()
                && is_test_file(f)
            {
                file_removed_tests += count_test_indicators(content);
            }
        }
    }

    finish_file(
        current_file.as_deref(),
        file_added_tests,
        file_removed_tests,
        &mut test_weakening,
    );

    if diff_loc > loc_budget {
        errors.push(format!(
            "Diff size of {diff_loc} LOC exceeds maximum budget of {loc_budget} LOC"
        ));
    } else if diff_loc > 300 {
        warnings.push(format!(
            "Diff size of {diff_loc} LOC exceeds recommended soft budget of 300 LOC"
        ));
    }

    if !allow_weakening {
        for w in &test_weakening {
            errors.push(w.clone());
        }
    } else {
        for w in &test_weakening {
            warnings.push(format!("(Allowed) {w}"));
        }
    }

    if !allow_escapes {
        for e in &escape_hatches {
            errors.push(e.clone());
        }
    } else {
        for e in &escape_hatches {
            warnings.push(format!("(Allowed) {e}"));
        }
    }

    (diff_loc, test_weakening, escape_hatches, errors, warnings)
}

pub fn check_boundaries(rules: &[BoundaryRule], edges: &[(String, String)]) -> Vec<String> {
    let mut violations = Vec::new();
    for (source, target) in edges {
        let norm_source = source.replace('\\', "/");
        for rule in rules {
            let from_pattern = rule.from.replace('\\', "/");
            if norm_source.contains(&from_pattern) {
                for disallow in &rule.disallow {
                    let pattern = disallow.trim_end_matches('*');
                    if target == disallow || target.starts_with(pattern) || target.contains(pattern)
                    {
                        violations.push(format!(
                            "Boundary violation in `{source}`: imports forbidden `{target}` (rule: from `{}` disallows `{disallow}`)",
                            rule.from
                        ));
                    }
                }
            }
        }
    }
    violations.sort();
    violations.dedup();
    violations
}

pub fn is_test_file(path: &str) -> bool {
    let p = path.to_lowercase().replace('\\', "/");
    p.contains("/tests/")
        || p.contains("/test/")
        || p.starts_with("tests/")
        || p.starts_with("test/")
        || p.ends_with("_test.go")
        || p.ends_with("_test.rs")
        || p.ends_with(".test.ts")
        || p.ends_with(".test.js")
        || p.ends_with(".test.tsx")
        || p.ends_with(".test.jsx")
        || p.ends_with(".spec.ts")
        || p.ends_with(".spec.js")
        || p.ends_with("_test.py")
        || p.split('/')
            .next_back()
            .is_some_and(|name| name.starts_with("test_"))
}

fn count_test_indicators(line: &str) -> usize {
    let mut count = 0;
    for indicator in [
        "assert",
        "expect",
        "t.Fatal",
        "t.Error",
        "#[test]",
        "def test_",
    ] {
        count += line.matches(indicator).count();
    }
    count
}

fn find_escape_hatch(line: &str) -> Option<&'static str> {
    [
        "TODO",
        "FIXME",
        "unimplemented!",
        "todo!",
        "NotImplementedError",
        ": any",
        "as any",
        "@ts-ignore",
        "typing.Any",
    ]
    .into_iter()
    .find(|&hatch| line.contains(hatch))
}

pub fn check_doc_drift(mappings: &[DocMapping], changed_files: &[String]) -> Vec<String> {
    let mut warnings = Vec::new();
    let normalized_changed: Vec<String> =
        changed_files.iter().map(|f| f.replace('\\', "/")).collect();

    for mapping in mappings {
        let code_pattern = mapping.code.replace('\\', "/");
        let doc_target = mapping.doc.replace('\\', "/");

        let code_matched = normalized_changed.iter().any(|file| {
            if code_pattern.ends_with("/**") {
                let prefix = &code_pattern[..code_pattern.len() - 3];
                file.starts_with(prefix) || file == prefix
            } else if code_pattern.ends_with("/*") {
                let prefix = &code_pattern[..code_pattern.len() - 2];
                file.starts_with(prefix)
            } else {
                file == &code_pattern || file.starts_with(&format!("{code_pattern}/"))
            }
        });

        if code_matched {
            let doc_updated = normalized_changed.iter().any(|file| file == &doc_target);
            if !doc_updated {
                warnings.push(format!(
                    "Documentation drift detected: '{}' was modified but corresponding doc '{}' was not updated",
                    mapping.code, mapping.doc
                ));
            }
        }
    }
    warnings
}
