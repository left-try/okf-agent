use okf_domain::gate::{BoundaryRule, check_boundaries, evaluate_diff};

#[test]
fn test_weakening_is_flagged_when_assertions_decrease_in_test_files() {
    let diff = r#"diff --git a/tests/unit_test.rs b/tests/unit_test.rs
--- a/tests/unit_test.rs
+++ b/tests/unit_test.rs
@@ -1,5 +1,3 @@
-assert_eq!(a, 1);
-assert_eq!(b, 2);
-#[test]
+assert_eq!(a, 1);
"#;

    let (loc, weakening, escapes, errors, _warnings) = evaluate_diff(diff, 400, false, false);
    assert_eq!(loc, 4);
    assert_eq!(weakening.len(), 1);
    assert!(weakening[0].contains("Test weakening in `tests/unit_test.rs`"));
    assert!(escapes.is_empty());
    assert!(!errors.is_empty());
}

#[test]
fn escape_hatches_and_stubs_are_detected() {
    let diff = r#"diff --git a/src/service.ts b/src/service.ts
--- a/src/service.ts
+++ b/src/service.ts
@@ -1,2 +1,6 @@
+const x: any = 1;
+const y = x as any;
+// TODO: implement this later
+// @ts-ignore
"#;

    let (loc, weakening, escapes, errors, _warnings) = evaluate_diff(diff, 400, false, false);
    assert_eq!(loc, 4);
    assert!(weakening.is_empty());
    assert_eq!(escapes.len(), 4);
    assert!(!errors.is_empty());
}

#[test]
fn diff_loc_budget_enforcement() {
    let mut large_diff = String::from(
        "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -0,0 +1,450 @@\n",
    );
    for i in 0..420 {
        large_diff.push_str(&format!("+let var_{i} = {i};\n"));
    }

    let (loc, _, _, errors, _) = evaluate_diff(&large_diff, 400, false, false);
    assert_eq!(loc, 420);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("exceeds maximum budget of 400 LOC"))
    );
}

#[test]
fn boundary_violations_are_detected_from_edges() {
    let rules = [BoundaryRule {
        from: "domain".into(),
        disallow: vec!["adapter".into(), "application".into()],
    }];
    let edges = [
        (
            "crates/domain/src/lib.rs".into(),
            "crates/adapter/src/db.rs".into(),
        ),
        ("crates/domain/src/lib.rs".into(), "serde".into()),
    ];

    let violations = check_boundaries(&rules, &edges);
    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("Boundary violation in `crates/domain/src/lib.rs`"));
}
