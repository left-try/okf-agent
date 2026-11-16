use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use okf_application::repository_index::{
    ImportEdge, IndexReport, IndexStore, IndexedContent, IndexedDocument, IndexedSymbol, SearchHit,
    SourceDocument,
};
use okf_domain::{
    diagnostics::{Diagnostic, ErrorKind},
    paths::RepoPath,
};
use regex::Regex;
use rusqlite::{Connection, params};

const SOURCE_SUFFIXES: &[&str] = &[
    "dart", "py", "js", "ts", "tsx", "jsx", "go", "rs", "java", "cs", "rb", "php", "slint",
];

static RUST_SYMBOL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;{}])(?:pub(?:\s*\([^)]*\))?\s+)?(?:async\s+|unsafe\s+|extern(?:\s+"[^"]*")?\s+|const\s+|default\s+)*(?:fn|struct|enum|trait|type|mod|const|static)\s+([A-Za-z_]\w*)"#)
        .expect("valid rust symbol regex")
});
static RUST_USE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;{}])(?:pub(?:\s*\([^)]*\))?\s+)?use\s+([A-Za-z0-9_:]+)"#)
        .expect("valid rust use regex")
});
static RUST_MOD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;{}])(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+([A-Za-z0-9_]+)\s*;"#)
        .expect("valid rust mod regex")
});

static PY_SYMBOL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*(?:def|class)\s+([A-Za-z_]\w*)"#).expect("valid py symbol regex")
});
static PY_FROM_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;])from\s+([A-Za-z0-9_.]+)\s+import"#).expect("valid py from regex")
});
static PY_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;])import\s+([A-Za-z0-9_.]+)"#).expect("valid py import regex")
});

static SLINT_SYMBOL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;{}])(?:export\s+)?(?:component|global|struct)\s+([A-Za-z_]\w*)"#)
        .expect("valid slint symbol regex")
});
static SLINT_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;{}])(?:import\s+.*?\s+)?from\s+["']([^"']+)["']"#)
        .expect("valid slint import regex")
});
static SLINT_IMPORT_DIRECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;{}])import\s+["']([^"']+)["']"#)
        .expect("valid slint import direct regex")
});

static JS_SYMBOL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;{}])(?:export\s+(?:default\s+)?)?(?:function\*?|class|interface|type)\s+([A-Za-z_]\w*)"#)
        .expect("valid js symbol regex")
});
static JS_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:from\s+["']([^"']+)["']|import\s+["']([^"']+)["']|require\s*\(\s*["']([^"']+)["']\s*\))"#)
        .expect("valid js import regex")
});

static GO_SYMBOL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;{}])(?:func(?:\s*\([^)]+\))?|type)\s+([A-Za-z_]\w*)"#)
        .expect("valid go symbol regex")
});
static GO_IMPORT_SINGLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;{}])import\s+["']([^"']+)["']"#).expect("valid go import regex")
});
static GO_IMPORT_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)import\s*\((.*?)\)"#).expect("valid go import block regex")
});
static STRING_LITERAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"["']([^"']+)["']"#).expect("valid string literal regex")
});

static DART_SYMBOL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;{}])(?:class|enum|mixin)\s+([A-Za-z_]\w*)"#)
        .expect("valid dart symbol regex")
});
static DART_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;{}])import\s+["']([^"']+)["']"#).expect("valid dart import regex")
});

static GENERAL_SYMBOL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:^|[\s;{}])(?:class|interface|enum|struct|def|function)\s+([A-Za-z_]\w*)"#)
        .expect("valid general symbol regex")
});
static GENERAL_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:from\s+["']([^"']+)["']|import\s+["']([^"']+)["']|require\s*\(\s*["']([^"']+)["']\s*\)|use\s+([A-Za-z0-9_:\\]+)|using\s+([A-Za-z0-9_.]+))"#)
        .expect("valid general import regex")
});

fn strip_c_comments_and_strings(source: &str, strip_strings: bool) -> String {
    let mut result = String::with_capacity(source.len());
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut idx = 0;
    let mut prev_char = ' ';

    while idx < len {
        let c = chars[idx];
        if c == '/' && idx + 1 < len {
            let next = chars[idx + 1];
            if next == '/' {
                idx += 2;
                while idx < len && chars[idx] != '\n' {
                    idx += 1;
                }
                if idx < len && chars[idx] == '\n' {
                    result.push('\n');
                    prev_char = '\n';
                    idx += 1;
                }
                continue;
            } else if next == '*' {
                idx += 2;
                while idx + 1 < len && !(chars[idx] == '*' && chars[idx + 1] == '/') {
                    if chars[idx] == '\n' {
                        result.push('\n');
                    }
                    idx += 1;
                }
                if idx + 1 < len {
                    idx += 2;
                }
                result.push(' ');
                prev_char = ' ';
                continue;
            }
        }

        if c == 'r' && !prev_char.is_alphanumeric() && prev_char != '_' && idx + 1 < len {
            let mut h_idx = idx + 1;
            while h_idx < len && chars[h_idx] == '#' {
                h_idx += 1;
            }
            if h_idx < len && chars[h_idx] == '"' {
                let hashes = h_idx - (idx + 1);
                idx = h_idx + 1;
                if strip_strings {
                    result.push_str("\"\"");
                } else {
                    result.push('r');
                    for _ in 0..hashes {
                        result.push('#');
                    }
                    result.push('"');
                }
                while idx < len {
                    if chars[idx] == '"' {
                        let mut match_hashes = 0;
                        while idx + 1 + match_hashes < len
                            && chars[idx + 1 + match_hashes] == '#'
                            && match_hashes < hashes
                        {
                            match_hashes += 1;
                        }
                        if match_hashes == hashes {
                            if !strip_strings {
                                result.push('"');
                                for _ in 0..hashes {
                                    result.push('#');
                                }
                            }
                            idx += 1 + hashes;
                            break;
                        }
                    }
                    if !strip_strings {
                        result.push(chars[idx]);
                    }
                    idx += 1;
                }
                prev_char = '"';
                continue;
            }
        }

        if c == '"' {
            idx += 1;
            if strip_strings {
                result.push_str("\"\"");
                let mut escaped = false;
                while idx < len {
                    let sc = chars[idx];
                    idx += 1;
                    if escaped {
                        escaped = false;
                    } else if sc == '\\' {
                        escaped = true;
                    } else if sc == '"' {
                        break;
                    }
                }
            } else {
                result.push('"');
                let mut escaped = false;
                while idx < len {
                    let sc = chars[idx];
                    result.push(sc);
                    idx += 1;
                    if escaped {
                        escaped = false;
                    } else if sc == '\\' {
                        escaped = true;
                    } else if sc == '"' {
                        break;
                    }
                }
            }
            prev_char = '"';
            continue;
        }

        if c == '\'' {
            let mut is_char_lit = false;
            let mut close_idx = 0;
            let mut check_idx = idx + 1;
            let mut escaped = false;
            while check_idx < len && check_idx <= idx + 8 && chars[check_idx] != '\n' {
                let sc = chars[check_idx];
                if escaped {
                    escaped = false;
                } else if sc == '\\' {
                    escaped = true;
                } else if sc == '\'' {
                    is_char_lit = true;
                    close_idx = check_idx;
                    break;
                }
                check_idx += 1;
            }

            if is_char_lit {
                if strip_strings {
                    idx = close_idx + 1;
                    result.push('\'');
                    result.push('\'');
                } else {
                    while idx <= close_idx {
                        result.push(chars[idx]);
                        idx += 1;
                    }
                }
                prev_char = '\'';
                continue;
            }
        }

        prev_char = c;
        result.push(c);
        idx += 1;
    }

    result
}

fn strip_python_comments_and_strings(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut idx = 0;

    while idx < len {
        let c = chars[idx];
        if c == '#' {
            idx += 1;
            while idx < len && chars[idx] != '\n' {
                idx += 1;
            }
            if idx < len && chars[idx] == '\n' {
                result.push('\n');
                idx += 1;
            }
            continue;
        }

        if c == '"' || c == '\'' {
            let quote = c;
            let is_triple = idx + 2 < len && chars[idx + 1] == quote && chars[idx + 2] == quote;
            if is_triple {
                idx += 3;
                while idx + 2 < len {
                    if chars[idx] == quote && chars[idx + 1] == quote && chars[idx + 2] == quote {
                        idx += 3;
                        break;
                    }
                    if chars[idx] == '\n' {
                        result.push('\n');
                    }
                    idx += 1;
                }
                result.push(' ');
                continue;
            } else {
                idx += 1;
                let mut escaped = false;
                while idx < len {
                    let sc = chars[idx];
                    idx += 1;
                    if escaped {
                        escaped = false;
                    } else if sc == '\\' {
                        escaped = true;
                    } else if sc == quote || sc == '\n' {
                        break;
                    }
                }
                result.push_str("\"\"");
                continue;
            }
        }

        result.push(c);
        idx += 1;
    }

    result
}

fn extract_symbols_and_edges(path: &str, content: &str) -> (Vec<String>, Vec<String>) {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !SOURCE_SUFFIXES.contains(&ext.as_str()) {
        return (Vec::new(), Vec::new());
    }

    let mut symbols = Vec::new();
    let mut edges = Vec::new();

    match ext.as_str() {
        "rs" => {
            let stripped = strip_c_comments_and_strings(content, true);
            for cap in RUST_SYMBOL.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    symbols.push(m.as_str().to_owned());
                }
            }
            for cap in RUST_USE.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    let target = m.as_str().trim_end_matches(':');
                    if !target.is_empty() {
                        edges.push(target.to_owned());
                    }
                }
            }
            for cap in RUST_MOD.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    edges.push(m.as_str().to_owned());
                }
            }
        }
        "py" => {
            let stripped = strip_python_comments_and_strings(content);
            for cap in PY_SYMBOL.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    symbols.push(m.as_str().to_owned());
                }
            }
            for cap in PY_FROM_IMPORT.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    edges.push(m.as_str().to_owned());
                }
            }
            for cap in PY_IMPORT.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    edges.push(m.as_str().to_owned());
                }
            }
        }
        "slint" => {
            let stripped = strip_c_comments_and_strings(content, false);
            for cap in SLINT_SYMBOL.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    symbols.push(m.as_str().to_owned());
                }
            }
            for cap in SLINT_IMPORT.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    edges.push(m.as_str().to_owned());
                }
            }
            for cap in SLINT_IMPORT_DIRECT.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    edges.push(m.as_str().to_owned());
                }
            }
        }
        "js" | "jsx" | "ts" | "tsx" => {
            let stripped = strip_c_comments_and_strings(content, false);
            for cap in JS_SYMBOL.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    symbols.push(m.as_str().to_owned());
                }
            }
            for cap in JS_IMPORT.captures_iter(&stripped) {
                for i in 1..=3 {
                    if let Some(m) = cap.get(i) {
                        edges.push(m.as_str().to_owned());
                        break;
                    }
                }
            }
        }
        "go" => {
            let stripped = strip_c_comments_and_strings(content, false);
            for cap in GO_SYMBOL.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    symbols.push(m.as_str().to_owned());
                }
            }
            for cap in GO_IMPORT_SINGLE.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    edges.push(m.as_str().to_owned());
                }
            }
            for block in GO_IMPORT_BLOCK.captures_iter(&stripped) {
                if let Some(b) = block.get(1) {
                    for cap in STRING_LITERAL.captures_iter(b.as_str()) {
                        if let Some(m) = cap.get(1) {
                            edges.push(m.as_str().to_owned());
                        }
                    }
                }
            }
        }
        "dart" => {
            let stripped = strip_c_comments_and_strings(content, false);
            for cap in DART_SYMBOL.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    symbols.push(m.as_str().to_owned());
                }
            }
            for cap in DART_IMPORT.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    edges.push(m.as_str().to_owned());
                }
            }
        }
        _ => {
            let stripped = strip_c_comments_and_strings(content, false);
            for cap in GENERAL_SYMBOL.captures_iter(&stripped) {
                if let Some(m) = cap.get(1) {
                    symbols.push(m.as_str().to_owned());
                }
            }
            for cap in GENERAL_IMPORT.captures_iter(&stripped) {
                for i in 1..=5 {
                    if let Some(m) = cap.get(i) {
                        edges.push(m.as_str().to_owned());
                        break;
                    }
                }
            }
        }
    }

    (symbols, edges)
}

#[derive(Default)]
pub struct SqliteIndexStore;

impl IndexStore for SqliteIndexStore {
    fn rebuild(
        &self,
        root: &Path,
        documents: &[SourceDocument],
    ) -> Result<IndexReport, Diagnostic> {
        let path = db_path(root)?;
        std::fs::create_dir_all(path.parent().expect("database path has parent"))
            .map_err(|error| io_error(&path, error))?;
        let mut db = Connection::open(&path).map_err(|error| db_error(&path, error))?;
        let tx = db.transaction().map_err(|error| db_error(&path, error))?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS documents(path TEXT PRIMARY KEY, content TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS symbols(name TEXT, path TEXT, UNIQUE(name,path));
             CREATE TABLE IF NOT EXISTS edges(source TEXT, target TEXT, kind TEXT, UNIQUE(source,target,kind));
             CREATE VIRTUAL TABLE IF NOT EXISTS document_search USING fts5(path, content);
             DELETE FROM documents; DELETE FROM symbols; DELETE FROM edges; DELETE FROM document_search;",
        ).map_err(|error| db_error(&path, error))?;
        let mut symbol_count = 0;
        for document in documents {
            tx.execute(
                "INSERT INTO documents(path,content) VALUES (?,?)",
                params![document.path, document.content],
            )
            .map_err(|error| db_error(&path, error))?;
            tx.execute(
                "INSERT INTO document_search(path,content) VALUES (?,?)",
                params![document.path, document.content],
            )
            .map_err(|error| db_error(&path, error))?;
            let (symbols, edges) = extract_symbols_and_edges(&document.path, &document.content);
            for name in symbols {
                let inserted = tx.execute(
                    "INSERT OR IGNORE INTO symbols(name,path) VALUES (?,?)",
                    params![name, document.path],
                )
                .map_err(|error| db_error(&path, error))?;
                if inserted > 0 {
                    symbol_count += 1;
                }
            }
            for target in edges {
                tx.execute(
                    "INSERT OR IGNORE INTO edges(source,target,kind) VALUES (?,?,'import')",
                    params![document.path, target],
                )
                .map_err(|error| db_error(&path, error))?;
            }
        }
        tx.commit().map_err(|error| db_error(&path, error))?;
        let free: i64 = db
            .pragma_query_value(None, "freelist_count", |row| row.get(0))
            .map_err(|error| db_error(&path, error))?;
        let pages: i64 = db
            .pragma_query_value(None, "page_count", |row| row.get(0))
            .map_err(|error| db_error(&path, error))?;
        let page_size: i64 = db
            .pragma_query_value(None, "page_size", |row| row.get(0))
            .map_err(|error| db_error(&path, error))?;
        if free.saturating_mul(page_size) >= 1_048_576 && free.saturating_mul(4) >= pages {
            db.execute_batch("VACUUM")
                .map_err(|error| db_error(&path, error))?;
        }
        Ok(IndexReport {
            files: documents.len(),
            symbols: symbol_count,
        })
    }

    fn search(&self, root: &Path, query: &str, limit: usize) -> Result<Vec<SearchHit>, Diagnostic> {
        let path = db_path(root)?;
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let db = Connection::open(&path).map_err(|error| db_error(&path, error))?;
        let fts_query = query.replace('\'', " ");
        let documents = match fts_search(&db, &fts_query, limit) {
            Ok(rows) => rows,
            Err(_) => db.prepare("SELECT path, substr(content,1,240) FROM documents WHERE content LIKE ? ORDER BY path LIMIT ?")
                .and_then(|mut statement| statement.query_map(params![format!("%{query}%"), limit_as_i64(limit)], |row| Ok(SearchHit { path: row.get(0)?, excerpt: row.get(1)? }))?.collect::<Result<Vec<_>,_>>())
                .map_err(|error| db_error(&path,error))?,
        };
        let symbols = db
            .prepare("SELECT name,path FROM symbols WHERE name LIKE ? ORDER BY name,path LIMIT ?")
            .and_then(|mut statement| {
                statement
                    .query_map(params![format!("%{query}%"), limit_as_i64(limit)], |row| {
                        Ok(SearchHit {
                            path: row.get(1)?,
                            excerpt: format!("symbol: {}", row.get::<_, String>(0)?),
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| db_error(&path, error))?;
        Ok(documents.into_iter().chain(symbols).collect())
    }

    fn indexed_content(&self, root: &Path) -> Result<IndexedContent, Diagnostic> {
        let path = db_path(root)?;
        if !path.is_file() {
            return Err(Diagnostic::new(
                ErrorKind::NotFound,
                "OKF index not found; run `okf-agent index` first",
            )
            .at_path(".okf-index/knowledge.sqlite3"));
        }
        let db = Connection::open(&path).map_err(|error| db_error(&path, error))?;
        let documents = db
            .prepare("SELECT path,content FROM documents ORDER BY path")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok(IndexedDocument {
                            path: row.get(0)?,
                            content: row.get(1)?,
                        })
                    })?
                    .collect()
            })
            .map_err(|error| db_error(&path, error))?;
        let symbols = db
            .prepare("SELECT name,path FROM symbols ORDER BY name,path")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok(IndexedSymbol {
                            name: row.get(0)?,
                            path: row.get(1)?,
                        })
                    })?
                    .collect()
            })
            .map_err(|error| db_error(&path, error))?;
        let edges = db
            .prepare("SELECT source,target,kind FROM edges ORDER BY source,target,kind")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok(ImportEdge {
                            source: row.get(0)?,
                            target: row.get(1)?,
                            kind: row.get(2)?,
                        })
                    })?
                    .collect()
            })
            .map_err(|error| db_error(&path, error))?;
        Ok(IndexedContent {
            documents,
            symbols,
            edges,
        })
    }
}

fn db_path(root: &Path) -> Result<PathBuf, Diagnostic> {
    let relative = RepoPath::new(root, Path::new(".okf-index/knowledge.sqlite3"))?;
    Ok(root.join(relative.as_path()))
}
fn limit_as_i64(limit: usize) -> i64 {
    limit.min(i64::MAX as usize) as i64
}
fn fts_search(db: &Connection, query: &str, limit: usize) -> rusqlite::Result<Vec<SearchHit>> {
    db.prepare("SELECT path, snippet(document_search, 1, '', '', '...', 16) FROM document_search WHERE document_search MATCH ? ORDER BY path LIMIT ?")?
        .query_map(params![query, limit_as_i64(limit)], |row| Ok(SearchHit { path: row.get(0)?, excerpt: row.get(1)? }))?
        .collect()
}
fn io_error(path: &Path, error: std::io::Error) -> Diagnostic {
    Diagnostic::new(ErrorKind::Io, error.to_string()).at_path(path.to_string_lossy().to_string())
}
fn db_error(path: &Path, error: rusqlite::Error) -> Diagnostic {
    Diagnostic::new(ErrorKind::Index, error.to_string()).at_path(path.to_string_lossy().to_string())
}
