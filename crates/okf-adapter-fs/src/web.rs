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
    let mut out = String::new();
    for line in md.lines() {
        let s = line.trim();
        if s.is_empty() {
            continue;
        }
        if let Some(h) = s.strip_prefix("### ") {
            out.push_str(&format!("<h3>{}</h3>", escape(h)));
        } else if let Some(h) = s.strip_prefix("## ") {
            out.push_str(&format!("<h2>{}</h2>", escape(h)));
        } else if let Some(h) = s.strip_prefix("# ") {
            out.push_str(&format!("<h1>{}</h1>", escape(h)));
        } else {
            out.push_str(&format!("<p>{}</p>", escape(s)));
        }
    }
    out
}
fn page(output: &Path, route: &str, allowed: &BTreeSet<String>) -> (u16, String) {
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
                "<!doctype html><meta charset=utf-8><title>OKF</title><nav><a href=\"/\">Repository wiki</a></nav><main>{body}</main>"
            );
            let _ = write!(
                stream,
                "HTTP/1.1 {status} {}\r\nContent-Type: text/html; charset=utf-8\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                if status == 200 { "OK" } else { "Not Found" },
                body.len(),
                body
            );
        });
    }
    Ok(())
}
