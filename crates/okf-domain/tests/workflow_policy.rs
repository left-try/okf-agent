use okf_domain::workflow::{RigorLevel, TaskType, WorkflowMethod, WorkflowPolicy};

#[test]
fn workflow_policy_accepts_the_existing_json_shape_and_defaults() {
    let policy: WorkflowPolicy = serde_json::from_str(
        r#"{
            "schema_version": 1,
            "enabled_profiles": ["sdd", "tdd", "gtdd"],
            "selection": "adaptive",
            "default_rigor": "standard",
            "defaults": {"feature": ["sdd", "tdd"]},
            "gtdd": {
                "execution": "recommend-only",
                "fallback": "sequential-adversarial-review"
            }
        }"#,
    )
    .unwrap();

    assert_eq!(policy.schema_version, 1);
    assert_eq!(policy.default_rigor, RigorLevel::Standard);
    assert_eq!(
        policy.defaults[&TaskType::Feature],
        vec![WorkflowMethod::Sdd, WorkflowMethod::Tdd]
    );
    assert_eq!(
        policy.defaults[&TaskType::Bugfix],
        vec![WorkflowMethod::Tdd]
    );
}

#[test]
fn workflow_policy_rejects_unknown_keys_and_missing_schema_version() {
    let unknown = r#"{"schema_version":1,"unexpected":true}"#;
    let missing_version = r#"{"enabled_profiles":["sdd"]}"#;

    assert!(serde_json::from_str::<WorkflowPolicy>(unknown).is_err());
    assert!(serde_json::from_str::<WorkflowPolicy>(missing_version).is_err());
}

#[test]
fn workflow_enums_reject_values_outside_the_existing_contract() {
    assert!(serde_json::from_str::<RigorLevel>("\"urgent\"").is_err());
    assert!(serde_json::from_str::<WorkflowMethod>("\"pairing\"").is_err());
}
