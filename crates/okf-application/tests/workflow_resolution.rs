use okf_application::workflow::resolve;
use okf_domain::workflow::{
    RigorLevel, TaskType, WorkflowAssessment, WorkflowCapabilities, WorkflowMethod, WorkflowPolicy,
};

#[test]
fn strict_feature_and_behavioral_docs_select_expected_methods_and_phases() {
    let strict = resolve(
        WorkflowPolicy::default(),
        WorkflowAssessment {
            primary_type: TaskType::Feature,
            risk: Some(RigorLevel::Strict),
            ..Default::default()
        },
        WorkflowCapabilities::default(),
    )
    .unwrap();
    assert_eq!(
        strict.effective_methods,
        vec![WorkflowMethod::Sdd, WorkflowMethod::Tdd]
    );
    assert!(
        strict
            .phases
            .iter()
            .any(|phase| phase == "Broader verification")
    );

    let docs = resolve(
        WorkflowPolicy::default(),
        WorkflowAssessment {
            primary_type: TaskType::Docs,
            behavioral: true,
            ..Default::default()
        },
        WorkflowCapabilities::default(),
    )
    .unwrap();
    assert_eq!(docs.effective_methods, vec![WorkflowMethod::Tdd]);
}

#[test]
fn high_impact_forces_strict_and_gtdd_falls_back_without_independent_roles() {
    let decision = resolve(
        WorkflowPolicy::default(),
        WorkflowAssessment {
            primary_type: TaskType::Bugfix,
            high_impact: true,
            independent_challenge: true,
            ..Default::default()
        },
        WorkflowCapabilities::default(),
    )
    .unwrap();
    assert_eq!(decision.rigor, RigorLevel::Strict);
    assert_eq!(
        decision.fallback.map(|f| format!("{f:?}")),
        Some("SequentialAdversarialReview".into())
    );
    assert!(
        decision
            .phases
            .iter()
            .any(|phase| phase == "Single-agent adversarial review")
    );
}
