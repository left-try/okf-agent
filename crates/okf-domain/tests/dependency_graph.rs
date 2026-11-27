use okf_domain::graph::DependencyGraph;

#[test]
fn test_blast_radius_calculation() {
    let mut g = DependencyGraph::new();
    // A imports B, B imports C (C -> B -> A in dependency chain)
    // Edge (from, to): A imports B, B imports C
    g.add_edge(
        "crates/app/src/main.rs",
        "crates/domain/src/lib.rs",
        "import",
    );
    g.add_edge(
        "crates/domain/src/lib.rs",
        "crates/core/src/lib.rs",
        "import",
    );

    // If core changes, domain and app are affected (blast radius)
    let affected = g.blast_radius("crates/core/src/lib.rs", None);
    assert_eq!(affected.len(), 2);
    assert!(affected.contains(&"crates/domain/src/lib.rs".to_string()));
    assert!(affected.contains(&"crates/app/src/main.rs".to_string()));

    // With depth limit 1
    let direct_only = g.blast_radius("crates/core/src/lib.rs", Some(1));
    assert_eq!(direct_only, vec!["crates/domain/src/lib.rs".to_string()]);
}

#[test]
fn test_cycle_detection_and_toposort() {
    let mut g = DependencyGraph::new();
    g.add_edge("A", "B", "import");
    g.add_edge("B", "C", "import");
    assert!(!g.has_cycles());
    assert!(g.topological_order().is_ok());

    // Introduce cycle
    g.add_edge("C", "A", "import");
    assert!(g.has_cycles());
    assert!(g.topological_order().is_err());
}
