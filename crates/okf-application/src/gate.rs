use std::path::Path;

use okf_domain::diagnostics::Diagnostic;
use okf_domain::gate::GateReport;

use crate::lifecycle::{GitPort, LifecycleFs};
use crate::repository_index::IndexStore;

pub fn run_gate(
    root: &Path,
    loc_budget: usize,
    allow_weakening: bool,
    allow_escapes: bool,
    git: &impl GitPort,
    fs: &impl LifecycleFs,
    index: &impl IndexStore,
) -> Result<GateReport, Diagnostic> {
    let diff = git.diff(root);
    let (diff_loc, test_weakening, escape_hatches, mut errors, mut warnings) =
        okf_domain::gate::evaluate_diff(&diff, loc_budget, allow_weakening, allow_escapes);

    let changed_files = git.changed_files(root);
    if let Ok(mappings) = fs.load_doc_mappings(root)
        && !mappings.is_empty()
    {
        let drift_warnings = okf_domain::gate::check_doc_drift(&mappings, &changed_files);
        warnings.extend(drift_warnings);
    }

    let mut boundary_violations = Vec::new();
    if let Ok(indexed) = index.indexed_content(root)
        && let Ok(rules) = fs.load_boundaries(root)
        && !rules.is_empty()
    {
        let edges: Vec<(String, String)> = indexed
            .edges
            .into_iter()
            .map(|e| (e.source, e.target))
            .collect();
        boundary_violations = okf_domain::gate::check_boundaries(&rules, &edges);
        for v in &boundary_violations {
            errors.push(v.clone());
        }
    }

    let ok = errors.is_empty();
    Ok(GateReport {
        ok,
        diff_loc,
        loc_budget,
        test_weakening,
        escape_hatches,
        boundary_violations,
        errors,
        warnings,
    })
}
