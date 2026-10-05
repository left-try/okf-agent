from __future__ import annotations

import re
import sqlite3
from pathlib import Path
from urllib.parse import unquote, urlsplit

from .discovery import files
from .docs import check_docs
from .knowledge import GENERATED_END, GENERATED_START
from .paths import REQUIRED_FILES, index_dir, okf_dir


MARKDOWN_LINK = re.compile(r"!?\[[^\]]*\]\((<[^>]+>|[^)\s]+)(?:\s+[^)]*)?\)")
MARKDOWN_LINK_START = re.compile(r"!?\[[^\]]*\]\(")


def _index_is_fresh(root: Path, store: Path) -> bool:
    db: sqlite3.Connection | None = None
    try:
        db = sqlite3.connect(store)
        indexed = dict(db.execute("SELECT path, content FROM documents"))
    except sqlite3.Error:
        return False
    finally:
        if db is not None:
            db.close()

    current: dict[str, str] = {}
    try:
        for path in files(root):
            current[path.relative_to(root).as_posix()] = path.read_text(
                encoding="utf-8", errors="ignore"
            )[:500_000]
    except OSError:
        return False
    return indexed == current


def _validate_local_links(root: Path, document: Path) -> list[str]:
    errors: list[str] = []
    text = document.read_text(encoding="utf-8", errors="ignore")
    in_fence = False
    for line_number, line in enumerate(text.splitlines(), 1):
        if re.match(r"^\s*(```|~~~)", line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        matches = list(MARKDOWN_LINK.finditer(line))
        for start in MARKDOWN_LINK_START.finditer(line):
            if not any(match.start() == start.start() for match in matches):
                errors.append(
                    f"Malformed Markdown link: {document.relative_to(root)}:{line_number} -> {line[start.start():].strip()}"
                )
        for match in matches:
            target = match.group(1).strip("<>")
            parsed = urlsplit(target)
            if parsed.scheme or parsed.netloc or not parsed.path:
                continue
            decoded = unquote(parsed.path)
            resolved = (document.parent / decoded).resolve()
            try:
                resolved.relative_to(root.resolve())
            except ValueError:
                errors.append(f"Markdown link escapes repository: {document.relative_to(root)} -> {target}")
                continue
            if not resolved.exists():
                errors.append(f"Broken Markdown link: {document.relative_to(root)} -> {target}")
    return errors


def validate(root: Path) -> dict[str, object]:
    errors: list[str] = []
    warnings: list[str] = []
    bundle = okf_dir(root)
    for relative in REQUIRED_FILES:
        path = bundle / relative
        if not path.exists():
            errors.append(f"Missing required knowledge file: .okf/{relative}")

    manifest = bundle / "manifest.yaml"
    if manifest.exists():
        text = manifest.read_text(encoding="utf-8", errors="ignore")
        versions = re.findall(r"^schema_version:\s*(\S+)\s*$", text, re.M)
        names = re.findall(r"^name:\s*(\S.*)$", text, re.M)
        if versions != ["1"] or len(names) != 1 or not names[0].strip():
            errors.append("manifest.yaml must contain one schema_version: 1 and one non-empty name")

    if bundle.exists():
        for doc in bundle.rglob("*.md"):
            text = doc.read_text(encoding="utf-8", errors="ignore")
            starts = [match.start() for match in re.finditer(re.escape(GENERATED_START), text)]
            ends = [match.start() for match in re.finditer(re.escape(GENERATED_END), text)]
            if len(starts) != len(ends) or len(starts) > 1 or (starts and starts[0] > ends[0]):
                errors.append(f"Malformed generated block: {doc.relative_to(root)}")
            errors.extend(_validate_local_links(root, doc))

    store = index_dir(root) / "knowledge.sqlite3"
    if not store.exists():
        warnings.append("Index is absent; run `okf-agent index`")
    elif not _index_is_fresh(root, store):
        warnings.append("Index is stale; run `okf-agent index` or `okf-agent update`")
    site = index_dir(root) / "site"
    if site.exists():
        docs_result = check_docs(root)
        errors.extend(str(error) for error in docs_result["errors"])
    return {"ok": not errors, "errors": errors, "warnings": warnings}
