use okf_adapter_mcp::{
    bundle::{BundleServer, GraphArgs, IndexArgs, SearchArgs as BundleSearchArgs},
    repository::{RepositoryServer, SearchArgs, read_concept},
};
use serde_json::json;
use std::path::PathBuf;

#[test]
fn repository_and_bundle_profiles_publish_the_existing_tool_names() {
    let repository = RepositoryServer::new(PathBuf::from("."));
    let mut names = repository.tool_names();
    names.sort();
    let mut expected = vec![
        "okf.get_change_context",
        "okf.get_concept",
        "okf.get_workflow_policy",
        "okf.initialize",
        "okf.record_decision",
        "okf.reindex",
        "okf.resolve_workflow",
        "okf.search",
        "okf.status",
        "okf.update_after_change",
        "okf.validate",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(names, expected);
    let bundle = BundleServer::new(PathBuf::from("bundle"), PathBuf::from("index"));
    let mut names = bundle.tool_names();
    names.sort();
    let mut expected = vec![
        "okf.bundle_backlinks",
        "okf.bundle_export",
        "okf.bundle_index",
        "okf.bundle_neighborhood",
        "okf.bundle_search",
        "okf.bundle_validate",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(names, expected);
}

#[test]
fn argument_defaults_match_python_tool_contract() {
    let search: SearchArgs = serde_json::from_value(json!({"query":"term"})).unwrap();
    assert_eq!(search.limit, 8);
    let index: IndexArgs = serde_json::from_value(json!({})).unwrap();
    assert!(!index.wikilinks);
    let search: BundleSearchArgs = serde_json::from_value(json!({"query":"term"})).unwrap();
    assert_eq!(search.limit, 10);
    let graph: GraphArgs = serde_json::from_value(json!({"path":"guide.md"})).unwrap();
    assert_eq!(graph.depth, 1);
}

#[test]
fn concept_reads_reject_paths_outside_the_knowledge_bundle() {
    let root = std::env::temp_dir().join(format!("okf-mcp-concept-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".okf")).unwrap();
    std::fs::write(root.join(".okf/topic.md"), "# Topic").unwrap();
    std::fs::write(root.join("secret.md"), "secret").unwrap();
    assert_eq!(read_concept(&root, "topic.md").unwrap(), "# Topic");
    assert!(read_concept(&root, "../../secret.md").is_err());
    assert!(read_concept(&root, "../secret.md").is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn workflow_tool_accepts_the_python_stateful_argument() {
    let args: okf_adapter_mcp::repository::ResolveArgs =
        serde_json::from_value(json!({"primary_type":"feature","stateful":true})).unwrap();
    assert!(args.stateful);
}
