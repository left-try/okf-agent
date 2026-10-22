use regex::Regex;
use std::sync::LazyLock;

static LINKS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"!?\[[^\]]*\]\(\s*(<[^>]*>|(?:\\.|[^\s)])+)(?:\s+[^)]*)?\s*\)"#)
        .expect("valid Markdown link regex")
});
static WIKILINKS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]]+)\]\]").expect("valid wikilink regex"));
static ESCAPES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\([\\`*_{}\[\]()#+\-.!|> ])").expect("valid Markdown escape regex")
});

pub fn markdown_targets(body: &str, wikilinks: bool) -> Vec<String> {
    let visible = remove_inline_code(&remove_fenced_code(body));
    let mut targets = LINKS
        .captures_iter(&visible)
        .filter_map(|capture| capture.get(1))
        .map(|target| target.as_str().trim_matches(['<', '>']))
        .map(unescape)
        .collect::<Vec<_>>();
    if wikilinks {
        targets.extend(
            WIKILINKS
                .captures_iter(&visible)
                .filter_map(|capture| {
                    let full = capture.get(0)?;
                    if full.start() > 0 && visible[..full.start()].ends_with('!') {
                        return None;
                    }
                    capture.get(1)
                })
                .filter_map(|capture| {
                    let target = capture
                        .as_str()
                        .split('|')
                        .next()
                        .unwrap_or("")
                        .split('#')
                        .next()
                        .unwrap_or("")
                        .trim();
                    (!target.is_empty()).then(|| unescape(target))
                }),
        );
    }
    targets
}

fn remove_fenced_code(body: &str) -> String {
    let mut result = String::new();
    let mut fence: Option<(char, usize)> = None;
    for line in body.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some((marker, count)) = fence {
            let closing = trimmed.chars().take_while(|ch| *ch == marker).count();
            if closing >= count && trimmed[closing..].trim().is_empty() {
                fence = None;
            }
            result.push(if line.ends_with('\n') { '\n' } else { ' ' });
            continue;
        }
        let marker = trimmed.chars().next();
        let count = marker
            .map(|marker| trimmed.chars().take_while(|ch| *ch == marker).count())
            .unwrap_or(0);
        if matches!(marker, Some('`' | '~')) && count >= 3 {
            fence = Some((marker.unwrap(), count));
            result.push(if line.ends_with('\n') { '\n' } else { ' ' });
        } else {
            result.push_str(line);
        }
    }
    result
}

fn remove_inline_code(body: &str) -> String {
    let chars: Vec<char> = body.chars().collect();
    let mut result = String::with_capacity(body.len());
    let mut index = 0;
    while index < chars.len() {
        if chars[index] != '`' {
            result.push(chars[index]);
            index += 1;
            continue;
        }
        let run = chars[index..].iter().take_while(|ch| **ch == '`').count();
        let close = (index + run..chars.len()).find(|start| {
            chars[*start] == '`'
                && chars[*start..].iter().take_while(|ch| **ch == '`').count() == run
        });
        if let Some(close) = close {
            for ch in &chars[index + run..close] {
                if *ch == '\n' {
                    result.push('\n');
                }
            }
            index = close + run;
        } else {
            result.extend(std::iter::repeat_n(' ', run));
            index += run;
        }
    }
    result
}

pub fn unescape(value: &str) -> String {
    ESCAPES.replace_all(value, "$1").into_owned()
}
