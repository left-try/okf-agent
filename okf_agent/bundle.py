from __future__ import annotations

"""Read-only OKF Markdown bundle indexing and conformance profile.

The bundle root is user-owned source. The index root is disposable machine data;
the profile never initializes repository instructions, hooks, or Git state.
"""

import hashlib
import json
import re
import sqlite3
from collections import defaultdict, deque
from datetime import date
from pathlib import Path, PurePosixPath
from urllib.parse import unquote, urlsplit

import yaml

PROFILE_CONTRACT = {"id": "okf-agent.bundle", "version": 1, "okf_version": "0.2"}

SCHEMA = """
CREATE TABLE IF NOT EXISTS documents (
  path TEXT PRIMARY KEY, title TEXT, doc_type TEXT, aliases TEXT NOT NULL,
  frontmatter TEXT NOT NULL, body TEXT NOT NULL, content_hash TEXT NOT NULL,
  revision INTEGER NOT NULL
);
CREATE VIRTUAL TABLE IF NOT EXISTS document_search USING fts5(path UNINDEXED, title, doc_type, aliases, body);
CREATE TABLE IF NOT EXISTS links (
  source TEXT NOT NULL, target TEXT, locator TEXT NOT NULL, relation TEXT,
  status TEXT NOT NULL, diagnostic TEXT
);
CREATE TABLE IF NOT EXISTS provenance (
  document TEXT NOT NULL, source_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS validation (payload TEXT NOT NULL);
"""
FENCE = re.compile(r"(?ms)^\s*(`{3,}|~{3,})[^\n]*\n.*?^\s*\1\s*$")
LINK = re.compile(r"!?\[[^\]]*\]\(\s*(<[^>]*>|(?:\\.|[^\s)])+)(?:\s+[^)]*)?\s*\)")
WIKILINK = re.compile(r"(?<!!)\[\[([^\]]+)\]\]")
MD_ESCAPE = re.compile(r"\\([\\`*_{}\[\]()#+\-.!|> ])")


def _rel(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def _parse(path: Path) -> tuple[dict, str, str | None]:
    text = path.read_bytes().decode("utf-8-sig")
    if not text.startswith("---\n") and not text.startswith("---\r\n"):
        return {}, text, "Missing YAML frontmatter"
    lines = text.splitlines(keepends=True)
    end = next((i for i in range(1, len(lines)) if lines[i].strip() == "---"), None)
    if end is None:
        return {}, text, "Unclosed YAML frontmatter"
    try:
        metadata = yaml.safe_load("".join(lines[1:end])) or {}
    except yaml.YAMLError as exc:
        return {}, "".join(lines[end + 1:]), f"Invalid YAML frontmatter: {exc}" 
    if not isinstance(metadata, dict):
        return {}, "".join(lines[end + 1:]), "YAML frontmatter must be a mapping"
    return metadata, "".join(lines[end + 1:]), None


def _markdown_targets(body: str, wikilinks: bool) -> list[str]:
    visible = FENCE.sub("", body)
    # Inline code is not link syntax. Keep line breaks to avoid crossing blocks.
    visible = re.sub(r"(`+)(.*?)\1", "", visible, flags=re.S)
    targets: list[str] = []
    for match in LINK.finditer(visible):
        target = match.group(1).strip("<>")
        targets.append(MD_ESCAPE.sub(r"\1", target))
    if wikilinks:
        for match in WIKILINK.finditer(visible):
            target = match.group(1).split("|", 1)[0].split("#", 1)[0].strip()
            if target:
                targets.append(target)
    return targets


def _all_markdown(root: Path) -> list[Path]:
    return sorted((p for p in root.rglob("*.md") if p.is_file() and not p.is_symlink()), key=lambda p: p.as_posix().casefold())


def _resolve_link(root: Path, source: Path, raw: str, known: set[str], basenames: dict[str, list[str]]) -> tuple[str | None, str, str | None]:
    parsed = urlsplit(raw)
    if parsed.scheme or parsed.netloc or raw.startswith("#"):
        return None, "external", None
    path = MD_ESCAPE.sub(r"\1", unquote(parsed.path)).replace("\\", "/")
    if not path:
        return _rel(source, root), "resolved", None
    candidate = PurePosixPath(path)
    if path.startswith("/"):
        candidate = PurePosixPath(path.lstrip("/"))
    else:
        candidate = PurePosixPath(_rel(source.parent, root)) / candidate
    normalized = str(PurePosixPath(candidate))
    parts: list[str] = []
    for part in normalized.split("/"):
        if part in ("", "."):
            continue
        if part == "..":
            if not parts:
                return None, "broken", f"Link escapes bundle root: {raw}"
            parts.pop()
        else:
            parts.append(part)
    normalized = "/".join(parts)
    candidates = [normalized]
    if not PurePosixPath(normalized).suffix:
        candidates.append(normalized + ".md")
        candidates.append(normalized + "/index.md")
    for target in candidates:
        if target in known:
            return target, "resolved", None
    basename = PurePosixPath(normalized).name.casefold()
    same_name = basenames.get(basename, [])
    if same_name:
        return None, "ambiguous", f"Ambiguous link {raw!r}; matching files: {', '.join(same_name)}"
    return None, "broken", f"Broken link: {raw}"


def _connect(index_root: Path) -> sqlite3.Connection:
    index_root.mkdir(parents=True, exist_ok=True)
    db = sqlite3.connect(index_root / "knowledge.sqlite3")
    db.executescript(SCHEMA)
    return db


def index_bundle(root: str | Path, index_root: str | Path, *, wikilinks: bool = False) -> dict:
    root, index_root = Path(root).expanduser().resolve(), Path(index_root).expanduser().resolve()
    if not root.is_dir():
        raise ValueError(f"Bundle root is not a directory: {root}")
    pages = _all_markdown(root)
    known = {_rel(p, root) for p in pages}
    by_base: dict[str, list[str]] = defaultdict(list)
    for name in known:
        by_base[PurePosixPath(name).name.casefold()].append(name)
    db = _connect(index_root)
    try:
        old = {p: (h, rev) for p, h, rev in db.execute("SELECT path, content_hash, revision FROM documents")}
        db.execute("DELETE FROM documents")
        db.execute("DELETE FROM document_search")
        db.execute("DELETE FROM links")
        db.execute("DELETE FROM provenance")
        db.execute("DELETE FROM validation")
        diagnostics: list[dict] = []
        for path in pages:
            relative = _rel(path, root)
            reserved_index = PurePosixPath(relative).name == "index.md"
            reserved_log = PurePosixPath(relative).name == "log.md"
            metadata, body, parse_error = _parse(path)
            raw = path.read_bytes()
            digest = hashlib.sha256(raw).hexdigest()
            revision = old.get(relative, (None, 0))[1] + (old.get(relative, (None, 0))[0] != digest)
            title = str(metadata.get("title") or "")
            doc_type = str(metadata.get("type") or "")
            aliases = metadata.get("aliases", [])
            if isinstance(aliases, str):
                aliases = [aliases]
            if not isinstance(aliases, list):
                aliases = []
            aliases = [str(x) for x in aliases]
            fm_json = json.dumps(metadata, ensure_ascii=False, sort_keys=True, default=str)
            db.execute("INSERT INTO documents VALUES(?,?,?,?,?,?,?,?)", (relative, title, doc_type, json.dumps(aliases, ensure_ascii=False), fm_json, body, digest, int(revision)))
            db.execute("INSERT INTO document_search VALUES(?,?,?,?,?)", (relative, title, doc_type, " ".join(aliases), body))
            if reserved_index:
                # index.md has no frontmatter except an optional root-level
                # okf_version declaration.
                if not parse_error and metadata and (relative != "index.md" or set(metadata) != {"okf_version"}):
                    diagnostics.append({"path": relative, "severity": "error", "message": "index.md frontmatter is only allowed at the bundle root and may contain only okf_version"})
                if parse_error and not parse_error.startswith("Missing YAML frontmatter"):
                    diagnostics.append({"path": relative, "severity": "error", "message": parse_error})
                if relative == "index.md" and metadata.get("okf_version") not in (None, "0.2"):
                    diagnostics.append({"path": relative, "severity": "warning", "message": "Unrecognized okf_version; consumed best-effort"})
                if not re.search(r"(?m)^#{1,6}\s+.+", body):
                    diagnostics.append({"path": relative, "severity": "error", "message": "index.md must contain at least one section heading"})
            elif reserved_log:
                if metadata:
                    diagnostics.append({"path": relative, "severity": "error", "message": "log.md must not contain frontmatter"})
                if parse_error and not parse_error.startswith("Missing YAML frontmatter"):
                    diagnostics.append({"path": relative, "severity": "error", "message": parse_error})
                dates = re.findall(r"(?m)^##\s+(\S+)\s*$", body)
                if any(not re.fullmatch(r"\d{4}-\d{2}-\d{2}", date) for date in dates):
                    diagnostics.append({"path": relative, "severity": "error", "message": "log.md date headings must use YYYY-MM-DD"})
                elif any(_invalid_date_heading(value) for value in dates):
                    diagnostics.append({"path": relative, "severity": "error", "message": "log.md date headings must be valid calendar dates"})
                elif dates != sorted(dates, reverse=True):
                    diagnostics.append({"path": relative, "severity": "error", "message": "log.md date headings must be newest first"})
                elif not dates:
                    diagnostics.append({"path": relative, "severity": "error", "message": "log.md must contain at least one date heading"})
            else:
                if parse_error:
                    diagnostics.append({"path": relative, "severity": "error", "message": parse_error})
                elif not isinstance(metadata.get("type"), str) or not metadata["type"].strip():
                    diagnostics.append({"path": relative, "severity": "error", "message": "Missing non-empty string type"})
            sources = metadata.get("sources", [])
            if isinstance(sources, (dict, str)):
                sources = [sources]
            if isinstance(sources, list):
                for source in sources:
                    db.execute("INSERT INTO provenance VALUES(?,?)", (relative, json.dumps(source, ensure_ascii=False, sort_keys=True, default=str)))
                    locator = source.get("resource") if isinstance(source, dict) else None
                    if isinstance(locator, str) and locator.strip():
                        provenance_target, provenance_status, _ = _resolve_link(root, path, locator, known, by_base)
                        # Scope descriptions are valid source locators too. Preserve
                        # those and absolute URLs as external provenance edges.
                        edge_source = provenance_target or locator
                        db.execute("INSERT INTO links VALUES(?,?,?,?,?,?)", (edge_source, relative, locator, "provenance", provenance_status if provenance_target else "external", None))
            for raw_target in _markdown_targets(body, wikilinks):
                target, status, diagnostic = _resolve_link(root, path, raw_target, known, by_base)
                relation = None
                relations = metadata.get("relations", [])
                if isinstance(relations, dict):
                    relations = [relations]
                if isinstance(relations, list):
                    for item in relations:
                        if not isinstance(item, dict):
                            continue
                        rel_target = item.get("target", item.get("path", item.get("resource")))
                        if rel_target in (raw_target, target):
                            relation = item.get("relation", item.get("type", item.get("kind")))
                            break
                db.execute("INSERT INTO links VALUES(?,?,?,?,?,?)", (relative, target, raw_target, str(relation) if relation is not None else None, status, diagnostic))
                if diagnostic:
                    diagnostics.append({"path": relative, "severity": "diagnostic", "message": diagnostic})
        errors = [d for d in diagnostics if d["severity"] == "error"]
        db.execute("INSERT INTO validation VALUES(?)", (json.dumps({"ok": not errors, "conformant": not errors, "diagnostics": diagnostics}, ensure_ascii=False),))
        db.commit()
    finally:
        db.close()
    conformance_errors = [d for d in diagnostics if d["severity"] == "error"]
    return {"profile_contract": PROFILE_CONTRACT, "root": str(root), "index_root": str(index_root), "documents": len(pages), "conformant": not conformance_errors, "diagnostics": diagnostics}


def validate_bundle(root: str | Path, index_root: str | Path) -> dict:
    result = index_bundle(root, index_root)
    return {"ok": result["conformant"], "conformant": result["conformant"], "documents": result["documents"], "diagnostics": result["diagnostics"], "errors": [d for d in result["diagnostics"] if d["severity"] == "error"], "warnings": [d for d in result["diagnostics"] if d["severity"] != "error"]}


def _invalid_date_heading(value: str) -> bool:
    try:
        date.fromisoformat(value)
    except ValueError:
        return True
    return False


def search_bundle(index_root: str | Path, query: str, limit: int = 10) -> list[dict]:
    db = sqlite3.connect(Path(index_root).expanduser().resolve() / "knowledge.sqlite3")
    try:
        rows = db.execute("SELECT path, snippet(document_search,4,'[',']','…',18), title, doc_type FROM document_search WHERE document_search MATCH ? LIMIT ?", (query, max(1, limit))).fetchall()
        return [{"path": p, "excerpt": snippet, "title": title, "type": typ} for p, snippet, title, typ in rows]
    finally:
        db.close()


def graph_query(index_root: str | Path, path: str, depth: int = 1, *, backlinks: bool = False) -> dict:
    path = PurePosixPath(path.replace("\\", "/")).as_posix().removeprefix("./")
    db = sqlite3.connect(Path(index_root).expanduser().resolve() / "knowledge.sqlite3")
    try:
        nodes: dict[str, dict] = {}
        edges: list[dict] = []
        seen_edges: set[tuple] = set()
        queue = deque([(path, 0)])
        visited = {path}
        while queue:
            current, distance = queue.popleft()
            row = db.execute("SELECT path,title,doc_type,aliases,frontmatter,body,content_hash,revision FROM documents WHERE path=?", (current,)).fetchone()
            if row:
                nodes[current] = dict(zip(("path","title","type","aliases","frontmatter","body","hash","revision"), row))
                nodes[current]["aliases"] = json.loads(nodes[current]["aliases"])
                nodes[current]["frontmatter"] = json.loads(nodes[current]["frontmatter"])
            if distance >= max(0, min(depth, 8)):
                continue
            sql = "SELECT source,target,locator,relation,status FROM links WHERE source=? AND target IS NOT NULL"
            params: tuple = (current,)
            if backlinks:
                sql = "SELECT source,target,locator,relation,status FROM links WHERE target=? AND status='resolved'"
            for source, target, locator, relation, status in db.execute(sql, params):
                edge = (source, target, locator, relation)
                if edge not in seen_edges:
                    seen_edges.add(edge); edges.append({"source": source, "target": target, "locator": locator, "relation": relation, "status": status})
                other = source if backlinks else target
                if other not in visited:
                    visited.add(other); queue.append((other, distance + 1))
        return {"nodes": list(nodes.values()), "edges": edges}
    finally:
        db.close()


def export_bundle(index_root: str | Path) -> dict:
    db = sqlite3.connect(Path(index_root).expanduser().resolve() / "knowledge.sqlite3")
    try:
        nodes = []
        for row in db.execute("SELECT path,title,doc_type,aliases,frontmatter,body,content_hash,revision FROM documents ORDER BY path"):
            path,title,typ,aliases,fm,body,digest,revision = row
            nodes.append({"path":path,"title":title,"type":typ,"aliases":json.loads(aliases),"frontmatter":json.loads(fm),"body":body,"hash":digest,"revision":revision})
        links = [{"source":a,"target":b,"locator":c,"relation":d,"status":e,"diagnostic":f} for a,b,c,d,e,f in db.execute("SELECT source,target,locator,relation,status,diagnostic FROM links ORDER BY source,locator")]
        provenance = [{"document":a,"source":json.loads(b)} for a,b in db.execute("SELECT document,source_json FROM provenance ORDER BY document")]
        row = db.execute("SELECT payload FROM validation LIMIT 1").fetchone()
        validation = json.loads(row[0]) if row else {"ok": False, "conformant": False, "diagnostics": [{"severity": "error", "message": "Bundle index has no validation result; reindex it"}]}
        return {"profile_contract": PROFILE_CONTRACT, "nodes":nodes,"links":links,"provenance":provenance,"validation":validation}
    finally:
        db.close()

