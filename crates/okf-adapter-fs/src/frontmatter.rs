use serde_json::Value;

pub fn parse_frontmatter(text: &str) -> (Value, String, Option<String>) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    if !text.starts_with("---\n") && !text.starts_with("---\r\n") {
        return (
            Value::Object(Default::default()),
            text.to_owned(),
            Some("Missing YAML frontmatter".to_owned()),
        );
    }
    let Some(end) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(i, line)| (line.trim() == "---").then_some(i))
    else {
        return (
            Value::Object(Default::default()),
            text.to_owned(),
            Some("Unclosed YAML frontmatter".to_owned()),
        );
    };
    let yaml = lines[1..end].concat();
    let body = lines[end + 1..].concat();
    match serde_saphyr::from_str::<Value>(&yaml) {
        Ok(Value::Null) => (Value::Object(Default::default()), body, None),
        Ok(value @ Value::Object(_)) => (value, body, None),
        Ok(_) => (
            Value::Object(Default::default()),
            body,
            Some("YAML frontmatter must be a mapping".to_owned()),
        ),
        Err(error) => (
            Value::Object(Default::default()),
            body,
            Some(format!("Invalid YAML frontmatter: {error}")),
        ),
    }
}
