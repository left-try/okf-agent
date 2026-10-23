use okf_domain::diagnostics::{Diagnostic, ErrorKind};

pub const GENERATED_START: &str = "<!-- okf:generated:start -->";
pub const GENERATED_END: &str = "<!-- okf:generated:end -->";

pub fn replace_generated(text: &str, body: &str) -> String {
    let tokens = [GENERATED_START, GENERATED_END];
    let mut matches = Vec::new();
    for token in tokens {
        for (index, _) in text.match_indices(token) {
            matches.push((index, token));
        }
    }
    matches.sort_by_key(|entry| entry.0);
    if matches.is_empty() {
        return format!(
            "{text}\n\n{GENERATED_START}\n{}\n{GENERATED_END}\n",
            body.trim_end()
        );
    }
    let mut output = String::new();
    let mut cursor = 0;
    let mut active = false;
    for (index, token) in matches {
        if token == GENERATED_START {
            if !active {
                output.push_str(&text[cursor..index]);
                output.push_str(GENERATED_START);
                output.push('\n');
                output.push_str(body.trim_end());
                output.push('\n');
                active = true;
            }
        } else if active {
            output.push_str(GENERATED_END);
            active = false;
        } else {
            output.push_str(&text[cursor..index]);
        }
        cursor = index + token.len();
    }
    if !active {
        output.push_str(&text[cursor..]);
    } else {
        output.push_str(GENERATED_END);
    }
    output
}

pub fn validate_generated(text: &str, path: &str) -> Result<(), Diagnostic> {
    let starts = text
        .match_indices(GENERATED_START)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let ends = text
        .match_indices(GENERATED_END)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if starts.len() != ends.len()
        || starts.len() > 1
        || starts
            .first()
            .zip(ends.first())
            .is_some_and(|(start, end)| start > end)
    {
        return Err(
            Diagnostic::new(ErrorKind::InvalidConfiguration, "malformed generated block")
                .at_path(path),
        );
    }
    Ok(())
}
