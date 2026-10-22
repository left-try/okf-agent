use okf_application::bundle::{RawPage, prepare};
use serde_json::json;

fn page(path: &str, targets: &[&str]) -> RawPage {
    RawPage {
        path: path.into(),
        metadata: json!({"type":"note"}),
        body: String::new(),
        digest: format!("hash-{path}"),
        parse_error: None,
        targets: targets.iter().map(|value| (*value).into()).collect(),
    }
}

#[test]
fn resolves_root_links_and_reports_ambiguous_and_escaping_targets() {
    let prepared = prepare(vec![
        RawPage {
            path: "index.md".into(),
            metadata: json!({}),
            body: "# Index".into(),
            digest: "hash-index".into(),
            parse_error: None,
            targets: vec![],
        },
        page("lesson one.md", &[]),
        page("a/lesson.md", &[]),
        page("b/lesson.md", &[]),
        page(
            "guide.md",
            &[
                "/lesson%20one.md#part",
                "lesson.md",
                "../outside.md",
                "https://example.test/page",
            ],
        ),
    ]);
    assert!(prepared.conformant);
    assert!(
        prepared.links.iter().any(
            |link| link.target.as_deref() == Some("lesson one.md") && link.status == "resolved"
        )
    );
    assert!(prepared.links.iter().any(|link| {
        link.status == "ambiguous"
            && link
                .diagnostic
                .as_deref()
                .is_some_and(|message| message.contains("Ambiguous link"))
    }));
    assert!(prepared.links.iter().any(|link| {
        link.status == "broken"
            && link
                .diagnostic
                .as_deref()
                .is_some_and(|message| message.contains("escapes bundle root"))
    }));
    assert!(
        prepared
            .links
            .iter()
            .any(|link| link.status == "external" && link.target.is_none())
    );
}
