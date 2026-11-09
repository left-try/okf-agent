use crate::docs::{self, MANIFEST};
use okf_domain::diagnostics::{Diagnostic, ErrorKind};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    net::{TcpListener, ToSocketAddrs},
    path::{Path, PathBuf},
    thread,
};

fn decode(value: &str) -> Option<String> {
    let b = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if i + 2 >= b.len() {
                return None;
            }
            let h = std::str::from_utf8(&b[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}
pub fn resolve_route(
    output: &Path,
    route: &str,
    allowed: &BTreeSet<String>,
) -> Option<(String, PathBuf)> {
    let decoded = decode(route.split('?').next()?.split('#').next()?)?;
    if decoded.contains('\\') || decoded.contains('\0') {
        return None;
    }
    let name = if decoded.is_empty() || decoded == "/" {
        "index.md".to_owned()
    } else {
        decoded.trim_start_matches('/').to_owned()
    };
    if name.split('/').any(|p| p == ".." || p == ".")
        || !name.ends_with(".md")
        || !allowed.contains(&name)
    {
        return None;
    }
    let base = output.canonicalize().ok()?;
    let path = output.join(&name);
    let mut cursor = output.to_path_buf();
    for part in Path::new(&name) {
        cursor.push(part);
        if fs::symlink_metadata(&cursor).is_ok_and(|m| m.file_type().is_symlink()) {
            return None;
        }
    }
    let canonical = path.canonicalize().ok()?;
    if !canonical.starts_with(base) || !canonical.is_file() {
        return None;
    }
    Some((name, canonical))
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn render(md: &str) -> String {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let events = Parser::new(md).map(|event| match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        Event::Start(Tag::Link { dest_url, .. }) if !safe_link(&dest_url) => Event::Text("".into()),
        Event::Start(Tag::Image { .. }) => Event::Text("".into()),
        Event::End(TagEnd::Image) => Event::Text("".into()),
        other => other,
    });
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, events);
    out
}
fn safe_link(url: &str) -> bool {
    let value = url.trim().to_ascii_lowercase();
    !value.starts_with("//")
        && (!value.contains(':')
            || value.starts_with("https:")
            || value.starts_with("http:")
            || value.starts_with("mailto:"))
}
fn page(output: &Path, route: &str, allowed: &BTreeSet<String>) -> (u16, String) {
    if let Some(query) = route.strip_prefix("/search?q=") {
        let query = decode(&query.replace('+', " ")).unwrap_or_default();
        let mut body = format!("<h1>Search results for {}</h1><ul>", escape(&query));
        if !query.trim().is_empty() {
            for name in allowed {
                if let Some((_, path)) = resolve_route(output, &format!("/{name}"), allowed)
                    && fs::read_to_string(path)
                        .is_ok_and(|text| text.to_lowercase().contains(&query.to_lowercase()))
                {
                    body.push_str(&format!(
                        "<li><a href=\"/{}\">{}</a></li>",
                        escape(name),
                        escape(name)
                    ));
                }
            }
        }
        body.push_str("</ul>");
        return (200, body);
    }
    let Some((_, path)) = resolve_route(output, route, allowed) else {
        return (404, "<h1>Page not found</h1>".into());
    };
    match fs::read_to_string(path) {
        Ok(md) => (200, render(&md)),
        Err(_) => (404, "<h1>Page not found</h1>".into()),
    }
}
pub fn serve(root: &Path, host: &str, port: u16) -> Result<(), Diagnostic> {
    let output = docs::output_path(root, None)?;
    if !docs::check(root, None)?.ok {
        return Err(Diagnostic::new(
            ErrorKind::InvalidConfiguration,
            "Documentation is missing or stale; run docs build first",
        ));
    }
    let data = fs::read(output.join(MANIFEST))
        .map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()))?;
    let value: serde_json::Value = serde_json::from_slice(&data)
        .map_err(|e| Diagnostic::new(ErrorKind::InvalidConfiguration, e.to_string()))?;
    let allowed = value["outputs"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, _)| k.clone())
        .collect::<BTreeSet<_>>();
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()))?
        .next()
        .ok_or_else(|| Diagnostic::new(ErrorKind::InvalidPath, "invalid listen address"))?;
    let listener =
        TcpListener::bind(addr).map_err(|e| Diagnostic::new(ErrorKind::Io, e.to_string()))?;
    println!(
        "OKF documentation: http://{}/",
        listener.local_addr().unwrap_or(addr)
    );
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let output = output.clone();
        let allowed = allowed.clone();
        thread::spawn(move || {
            let mut request = [0u8; 8192];
            let Ok(size) = stream.read(&mut request) else {
                return;
            };
            let request = String::from_utf8_lossy(&request[..size]);
            let route = request
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or("/");
            let (status, body) = page(&output, route, &allowed);
            let body = format!(
                "<!doctype html><meta charset=utf-8><title>OKF</title><nav><a href=\"/\">Repository wiki</a><form action=\"/search\" method=\"get\"><label>Search <input name=\"q\" type=\"search\"></label><button>Search</button></form></nav><main>{body}</main>"
            );
            let _ = write!(
                stream,
                "HTTP/1.1 {status} {}\r\nContent-Type: text/html; charset=utf-8\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'self'\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                if status == 200 { "OK" } else { "Not Found" },
                body.len(),
                body
            );
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn markdown_links_are_clickable_and_raw_html_is_inert() {
        let html = render("- [Code Map](code-map.md)\n\n<script>alert(1)</script>\n");
        assert!(html.contains("href=\"code-map.md\""), "{html}");
        assert!(!html.contains("<script>"), "{html}");
    }
    #[test]
    fn wiki_search_finds_allowed_page_text() {
        let root = std::env::temp_dir().join(format!("okf-web-search-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("index.md"), "# searchable quasar").unwrap();
        let allowed = ["index.md".to_owned()].into_iter().collect();
        let (_, body) = page(&root, "/search?q=quasar", &allowed);
        assert!(body.contains("index.md"), "{body}");
        fs::remove_dir_all(root).unwrap();
    }
}
