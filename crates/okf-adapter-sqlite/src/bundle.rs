use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::Path,
};

use okf_application::bundle::{
    BundleIndexReport, BundlePage, BundleSearchHit, BundleStore, PreparedBundle,
};
use okf_domain::diagnostics::{Diagnostic, ErrorKind};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

#[derive(Default)]
pub struct SqliteBundleStore;

impl BundleStore for SqliteBundleStore {
    fn rebuild(
        &self,
        index_root: &Path,
        root_label: &str,
        prepared: &PreparedBundle,
    ) -> Result<BundleIndexReport, Diagnostic> {
        std::fs::create_dir_all(index_root).map_err(|error| io_error(index_root, error))?;
        let path = index_root.join("knowledge.sqlite3");
        let mut db = Connection::open(&path).map_err(|error| db_error(&path, error))?;
        let tx = db.transaction().map_err(|error| db_error(&path, error))?;
        tx.execute_batch("CREATE TABLE IF NOT EXISTS documents (path TEXT PRIMARY KEY, title TEXT, doc_type TEXT, aliases TEXT NOT NULL, frontmatter TEXT NOT NULL, body TEXT NOT NULL, content_hash TEXT NOT NULL, revision INTEGER NOT NULL);
CREATE VIRTUAL TABLE IF NOT EXISTS document_search USING fts5(path UNINDEXED, title, doc_type, aliases, body);
CREATE TABLE IF NOT EXISTS links (source TEXT NOT NULL, target TEXT, locator TEXT NOT NULL, relation TEXT, status TEXT NOT NULL, diagnostic TEXT);
CREATE TABLE IF NOT EXISTS provenance (document TEXT NOT NULL, source_json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS validation (payload TEXT NOT NULL);").map_err(|error| db_error(&path, error))?;
        let old_revisions = {
            let mut statement = tx
                .prepare("SELECT path,content_hash,revision FROM documents")
                .map_err(|error| db_error(&path, error))?;
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        (row.get::<_, String>(1)?, row.get::<_, i64>(2)?),
                    ))
                })
                .map_err(|error| db_error(&path, error))?
                .collect::<Result<HashMap<_, _>, _>>()
                .map_err(|error| db_error(&path, error))?
        };
        tx.execute_batch("DELETE FROM documents; DELETE FROM document_search; DELETE FROM links; DELETE FROM provenance; DELETE FROM validation;").map_err(|error| db_error(&path,error))?;
        for page in &prepared.pages {
            insert_page(&tx, page, old_revisions.get(&page.path), &path)?;
        }
        for link in &prepared.links {
            tx.execute(
                "INSERT INTO links VALUES(?,?,?,?,?,?)",
                params![
                    link.source,
                    link.target,
                    link.locator,
                    link.relation,
                    link.status,
                    link.diagnostic
                ],
            )
            .map_err(|error| db_error(&path, error))?;
        }
        for source in &prepared.provenance {
            let serialized = serde_json::to_string(&source.source)
                .map_err(|error| Diagnostic::new(ErrorKind::Internal, error.to_string()))?;
            tx.execute(
                "INSERT INTO provenance VALUES(?,?)",
                params![source.document, serialized],
            )
            .map_err(|error| db_error(&path, error))?;
        }
        let payload = json!({"ok": prepared.conformant, "conformant": prepared.conformant, "diagnostics": prepared.diagnostics});
        tx.execute("INSERT INTO validation VALUES(?)", [payload.to_string()])
            .map_err(|error| db_error(&path, error))?;
        tx.commit().map_err(|error| db_error(&path, error))?;
        Ok(BundleIndexReport {
            profile_contract: json!({"id":"okf-agent.bundle", "version":1, "okf_version":"0.2"}),
            root: root_label.to_owned(),
            index_root: index_root.to_string_lossy().into_owned(),
            documents: prepared.pages.len(),
            conformant: prepared.conformant,
            diagnostics: prepared.diagnostics.clone(),
        })
    }

    fn search(
        &self,
        index_root: &Path,
        query: &str,
        limit: usize,
    ) -> Result<Vec<BundleSearchHit>, Diagnostic> {
        let path = index_root.join("knowledge.sqlite3");
        let db = open_existing(&path)?;
        let mut statement = db.prepare("SELECT path,snippet(document_search,4,'[',']','…',18),title,doc_type FROM document_search WHERE document_search MATCH ? LIMIT ?").map_err(|error| db_error(&path,error))?;
        statement
            .query_map(
                params![query, limit.max(1).min(i64::MAX as usize) as i64],
                |row| {
                    Ok(BundleSearchHit {
                        path: row.get(0)?,
                        excerpt: row.get(1)?,
                        title: row.get(2)?,
                        doc_type: row.get(3)?,
                    })
                },
            )
            .map_err(|error| db_error(&path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| db_error(&path, error))
    }

    fn graph(
        &self,
        index_root: &Path,
        requested_path: &str,
        depth: usize,
        backlinks: bool,
    ) -> Result<Value, Diagnostic> {
        let path = index_root.join("knowledge.sqlite3");
        let db = open_existing(&path)?;
        let requested_path = requested_path
            .replace('\\', "/")
            .strip_prefix("./")
            .unwrap_or(&requested_path.replace('\\', "/"))
            .to_owned();
        let mut nodes = Vec::<Value>::new();
        let mut edges = Vec::<Value>::new();
        let mut edge_seen = HashSet::<(String, String, String, Option<String>)>::new();
        let mut queue = VecDeque::from([(requested_path.clone(), 0usize)]);
        let mut visited = HashSet::from([requested_path]);
        while let Some((current, distance)) = queue.pop_front() {
            let row = db.query_row("SELECT path,title,doc_type,aliases,frontmatter,body,content_hash,revision FROM documents WHERE path=?", [&current], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?,row.get::<_,String>(5)?,row.get::<_,String>(6)?,row.get::<_,i64>(7)?))).optional().map_err(|error| db_error(&path,error))?;
            if let Some((doc_path, title, doc_type, aliases, frontmatter, body, hash, revision)) =
                row
            {
                nodes.push(json!({"path":doc_path,"title":title,"type":doc_type,"aliases":serde_json::from_str::<Value>(&aliases).unwrap_or(json!([])),"frontmatter":serde_json::from_str::<Value>(&frontmatter).unwrap_or(json!({})),"body":body,"hash":hash,"revision":revision}));
            }
            if distance >= depth.min(8) {
                continue;
            }
            let sql = if backlinks {
                "SELECT source,target,locator,relation,status FROM links WHERE target=? AND status='resolved'"
            } else {
                "SELECT source,target,locator,relation,status FROM links WHERE source=? AND target IS NOT NULL"
            };
            let mut statement = db.prepare(sql).map_err(|error| db_error(&path, error))?;
            let rows = statement
                .query_map([&current], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .map_err(|error| db_error(&path, error))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| db_error(&path, error))?;
            for (source, target, locator, relation, status) in rows {
                let Some(target) = target else {
                    continue;
                };
                if edge_seen.insert((
                    source.clone(),
                    target.clone(),
                    locator.clone(),
                    relation.clone(),
                )) {
                    edges.push(json!({"source":source,"target":target,"locator":locator,"relation":relation,"status":status}));
                }
                let other = if backlinks { source } else { target };
                if visited.insert(other.clone()) {
                    queue.push_back((other, distance + 1));
                }
            }
        }
        Ok(json!({"nodes":nodes,"edges":edges}))
    }

    fn export(&self, index_root: &Path) -> Result<Value, Diagnostic> {
        let path = index_root.join("knowledge.sqlite3");
        let db = open_existing(&path)?;
        let mut nodes = Vec::new();
        {
            let mut statement=db.prepare("SELECT path,title,doc_type,aliases,frontmatter,body,content_hash,revision FROM documents ORDER BY path").map_err(|e|db_error(&path,e))?;
            for row in statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                    ))
                })
                .map_err(|e| db_error(&path, e))?
            {
                let (path, title, doc_type, aliases, frontmatter, body, hash, revision) =
                    row.map_err(|e| db_error(&path, e))?;
                nodes.push(json!({"path":path,"title":title,"type":doc_type,"aliases":json_parse(&aliases),"frontmatter":json_parse(&frontmatter),"body":body,"hash":hash,"revision":revision}));
            }
        }
        let links = query_values(
            &db,
            &path,
            "SELECT source,target,locator,relation,status,diagnostic FROM links ORDER BY source,locator",
            |row| {
                Ok(
                    json!({"source":row.get::<_,String>(0)?,"target":row.get::<_,Option<String>>(1)?,"locator":row.get::<_,String>(2)?,"relation":row.get::<_,Option<String>>(3)?,"status":row.get::<_,String>(4)?,"diagnostic":row.get::<_,Option<String>>(5)?}),
                )
            },
        )?;
        let provenance = query_values(
            &db,
            &path,
            "SELECT document,source_json FROM provenance ORDER BY document",
            |row| {
                Ok(
                    json!({"document":row.get::<_,String>(0)?,"source":json_parse(&row.get::<_,String>(1)?)}),
                )
            },
        )?;
        let validation = db.query_row("SELECT payload FROM validation LIMIT 1",[],|row|row.get::<_,String>(0)).map(|value|json_parse(&value)).unwrap_or_else(|_|json!({"ok":false,"conformant":false,"diagnostics":[{"severity":"error","message":"Bundle index has no validation result; reindex it"}]}));
        Ok(
            json!({"profile_contract":{"id":"okf-agent.bundle","version":1,"okf_version":"0.2"},"nodes":nodes,"links":links,"provenance":provenance,"validation":validation}),
        )
    }
}

fn insert_page(
    tx: &rusqlite::Transaction<'_>,
    page: &BundlePage,
    old: Option<&(String, i64)>,
    db_path: &Path,
) -> Result<(), Diagnostic> {
    let revision = old.map_or(1, |(hash, revision)| {
        revision + i64::from(hash != &page.content_hash)
    });
    let aliases = serde_json::to_string(&page.aliases)
        .map_err(|e| Diagnostic::new(ErrorKind::Internal, e.to_string()))?;
    let frontmatter = serde_json::to_string(&page.frontmatter)
        .map_err(|e| Diagnostic::new(ErrorKind::Internal, e.to_string()))?;
    tx.execute(
        "INSERT INTO documents VALUES(?,?,?,?,?,?,?,?)",
        params![
            page.path,
            page.title,
            page.doc_type,
            aliases,
            frontmatter,
            page.body,
            page.content_hash,
            revision
        ],
    )
    .map_err(|error| db_error(db_path, error))?;
    tx.execute(
        "INSERT INTO document_search VALUES(?,?,?,?,?)",
        params![
            page.path,
            page.title,
            page.doc_type,
            page.aliases.join(" "),
            page.body
        ],
    )
    .map_err(|error| db_error(db_path, error))?;
    Ok(())
}
fn open_existing(path: &Path) -> Result<Connection, Diagnostic> {
    if !path.is_file() {
        return Err(Diagnostic::new(
            ErrorKind::NotFound,
            "Bundle index not found; run bundle index first",
        )
        .at_path(path.to_string_lossy().to_string()));
    }
    Connection::open(path).map_err(|e| db_error(path, e))
}
fn json_parse(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or(Value::Null)
}
fn query_values<F>(
    db: &Connection,
    path: &Path,
    sql: &str,
    mut map: F,
) -> Result<Vec<Value>, Diagnostic>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Value>,
{
    let mut statement = db.prepare(sql).map_err(|e| db_error(path, e))?;
    statement
        .query_map([], |row| map(row))
        .map_err(|e| db_error(path, e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_error(path, e))
}
fn db_error(path: &Path, error: rusqlite::Error) -> Diagnostic {
    Diagnostic::new(ErrorKind::Index, error.to_string()).at_path(path.to_string_lossy().to_string())
}
fn io_error(path: &Path, error: std::io::Error) -> Diagnostic {
    Diagnostic::new(ErrorKind::Io, error.to_string()).at_path(path.to_string_lossy().to_string())
}
