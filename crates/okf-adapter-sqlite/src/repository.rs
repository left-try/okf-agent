use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use okf_application::repository_index::{
    ImportEdge, IndexReport, IndexStore, IndexedContent, IndexedDocument, IndexedSymbol, SearchHit,
    SourceDocument,
};
use okf_domain::diagnostics::{Diagnostic, ErrorKind};
use regex::Regex;
use rusqlite::{Connection, params};

static SYMBOL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:def|class|function|interface|type|func|struct)\s+([A-Za-z_]\w*)")
        .expect("valid symbol regex")
});
static IMPORT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:from\s+([\w.]+)|import\s+([\w./-]+)|require\(['"]([^'"]+))"#)
        .expect("valid import regex")
});

#[derive(Default)]
pub struct SqliteIndexStore;

impl IndexStore for SqliteIndexStore {
    fn rebuild(
        &self,
        root: &Path,
        documents: &[SourceDocument],
    ) -> Result<IndexReport, Diagnostic> {
        let path = db_path(root);
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
            for capture in SYMBOL_PATTERN.captures_iter(&document.content) {
                let name = capture.get(1).expect("symbol capture exists").as_str();
                tx.execute(
                    "INSERT OR IGNORE INTO symbols(name,path) VALUES (?,?)",
                    params![name, document.path],
                )
                .map_err(|error| db_error(&path, error))?;
                symbol_count += 1;
            }
            for captures in IMPORT_PATTERN.captures_iter(&document.content) {
                if let Some(target) = (1..=3)
                    .find_map(|index| captures.get(index))
                    .map(|capture| capture.as_str())
                {
                    tx.execute(
                        "INSERT OR IGNORE INTO edges(source,target,kind) VALUES (?,?,'import')",
                        params![document.path, target],
                    )
                    .map_err(|error| db_error(&path, error))?;
                }
            }
        }
        tx.commit().map_err(|error| db_error(&path, error))?;
        Ok(IndexReport {
            files: documents.len(),
            symbols: symbol_count,
        })
    }

    fn search(&self, root: &Path, query: &str, limit: usize) -> Result<Vec<SearchHit>, Diagnostic> {
        let path = db_path(root);
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
        let path = db_path(root);
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

fn db_path(root: &Path) -> PathBuf {
    root.join(".okf-index").join("knowledge.sqlite3")
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
