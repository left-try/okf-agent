use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use okf_adapter_fs::yaml::parse_yaml;
use okf_domain::paths::RepoPath;
use serde::Deserialize;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "okf-agent-yaml-test-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug, Deserialize, PartialEq)]
struct YamlFixture {
    name: String,
    enabled: bool,
}

#[test]
fn yaml_parser_deserializes_typed_values() {
    let scratch = Scratch::new();
    let path = RepoPath::new(&scratch.0, std::path::Path::new("manifest.yaml")).unwrap();

    let parsed: YamlFixture = parse_yaml("name: okf\nenabled: true\n", &path).unwrap();

    assert_eq!(
        parsed,
        YamlFixture {
            name: "okf".into(),
            enabled: true
        }
    );
}

#[test]
fn malformed_yaml_returns_a_path_aware_configuration_diagnostic() {
    let scratch = Scratch::new();
    let path = RepoPath::new(&scratch.0, std::path::Path::new("manifest.yaml")).unwrap();

    let error = parse_yaml::<YamlFixture>("name: [unterminated\n", &path).unwrap_err();

    assert_eq!(
        error.kind,
        okf_domain::diagnostics::ErrorKind::InvalidConfiguration
    );
    assert_eq!(error.path.as_deref(), Some("manifest.yaml"));
    assert!(error.message.contains("line") || error.message.contains("column"));
}
