from __future__ import annotations

import json
import os
import re
import sqlite3
from pathlib import Path

from .paths import index_dir

SKIP = {".git", ".okf", ".okf-index", "node_modules", ".venv", "venv", "dist", "build", "__pycache__"}
SOURCE_SUFFIXES = {".py", ".js", ".ts", ".tsx", ".jsx", ".go", ".rs", ".java", ".cs", ".rb", ".php"}


def files(root: Path):
    for base, dirs, names in os.walk(root):
        dirs[:] = [d for d in dirs if d not in SKIP]
        for name in names:
            path = Path(base) / name
            if path.suffix.lower() in SOURCE_SUFFIXES or name in {"package.json", "pyproject.toml", "requirements.txt", "Dockerfile", "docker-compose.yml", ".env.example"}:
                yield path


def detect(root: Path) -> dict[str, list[str]]:
    found: dict[str, list[str]] = {"languages": [], "framework_hints": [], "manifests": []}
    names = {p.name for p in root.rglob("*") if p.is_file() and not any(part in SKIP for part in p.parts)}
    mapping = {"pyproject.toml": ("Python", None), "requirements.txt": ("Python", None), "package.json": ("JavaScript", "Node.js"), "go.mod": ("Go", None), "Cargo.toml": ("Rust", None), "pom.xml": ("Java", None)}
    for name, (language, framework) in mapping.items():
        if name in names:
            found["languages"].append(language)
            found["manifests"].append(name)
            if framework:
                found["framework_hints"].append(framework)
    return {key: sorted(set(value)) for key, value in found.items()}


def symbols(path: Path) -> list[str]:
    try:
        text = path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return []
    return re.findall(r"(?:def|class|function|interface|type|func|struct)\s+([A-Za-z_]\w*)", text)


def build_index(root: Path) -> dict[str, int]:
    store = index_dir(root) / "knowledge.sqlite3"
    store.parent.mkdir(exist_ok=True)
    db = sqlite3.connect(store)
    db.executescript("""
    CREATE TABLE IF NOT EXISTS documents(path TEXT PRIMARY KEY, content TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS symbols(name TEXT, path TEXT, UNIQUE(name,path));
    CREATE TABLE IF NOT EXISTS edges(source TEXT, target TEXT, kind TEXT, UNIQUE(source,target,kind));
    CREATE VIRTUAL TABLE IF NOT EXISTS document_search USING fts5(path, content);
    DELETE FROM documents; DELETE FROM symbols; DELETE FROM edges; DELETE FROM document_search;
    """)
    file_count = symbol_count = 0
    for path in files(root):
        relative = path.relative_to(root).as_posix()
        content = path.read_text(encoding="utf-8", errors="ignore")[:500_000]
        db.execute("INSERT INTO documents VALUES (?, ?)", (relative, content))
        db.execute("INSERT INTO document_search VALUES (?, ?)", (relative, content))
        file_count += 1
        for name in symbols(path):
            db.execute("INSERT OR IGNORE INTO symbols VALUES (?, ?)", (name, relative))
            symbol_count += 1
        for match in re.findall(r"(?:from\s+([\w.]+)|import\s+([\w./-]+)|require\(['\"]([^'\"]+))", content):
            target = next((value for value in match if value), "")
            if target:
                db.execute("INSERT OR IGNORE INTO edges VALUES (?, ?, 'import')", (relative, target))
    db.commit(); db.close()
    return {"files": file_count, "symbols": symbol_count}


def search(root: Path, query: str, limit: int = 8) -> list[dict[str, str]]:
    store = index_dir(root) / "knowledge.sqlite3"
    if not store.exists(): return []
    db = sqlite3.connect(store)
    try:
        rows = db.execute("SELECT path, snippet(document_search, 1, '', '', '...', 16) FROM document_search WHERE document_search MATCH ? LIMIT ?", (query.replace("'", " "), limit)).fetchall()
    except sqlite3.OperationalError:
        rows = db.execute("SELECT path, substr(content, 1, 240) FROM documents WHERE content LIKE ? LIMIT ?", (f"%{query}%", limit)).fetchall()
    symbols_found = db.execute("SELECT name, path FROM symbols WHERE name LIKE ? LIMIT ?", (f"%{query}%", limit)).fetchall()
    db.close()
    return [{"path": row[0], "excerpt": row[1]} for row in rows] + [{"path": row[1], "excerpt": f"symbol: {row[0]}"} for row in symbols_found]
