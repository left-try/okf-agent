use serde_json::json;

use okf_adapter_fs::{frontmatter::parse_frontmatter, markdown_links::markdown_targets};

#[test]
fn frontmatter_keeps_unknown_metadata_and_reports_parse_errors_with_the_body() {
    let (metadata, body, error) =
        parse_frontmatter("\u{feff}---\ntitle: Intro\ncustom: [one, two]\n---\n# Body\n");
    assert_eq!(metadata["custom"], json!(["one", "two"]));
    assert_eq!(body, "# Body\n");
    assert!(error.is_none());

    let (metadata, body, error) = parse_frontmatter("---\nunknown: [\n---\nBody\n");
    assert!(metadata.as_object().unwrap().is_empty());
    assert_eq!(body, "Body\n");
    assert!(error.unwrap().starts_with("Invalid YAML frontmatter:"));
}

#[test]
fn markdown_targets_ignore_code_and_opt_in_wikilinks() {
    let body = "[relative](chapter%20one.md#part) ![image](./img.png)\n`[inline](no.md)`\n```md\n[code](no.md)\n```\n[[Wiki|label]] ![[image.png]]\n";
    assert_eq!(
        markdown_targets(body, false),
        ["chapter%20one.md#part", "./img.png"]
    );
    assert_eq!(
        markdown_targets(body, true),
        ["chapter%20one.md#part", "./img.png", "Wiki"]
    );
}
