from __future__ import annotations

import re
from pathlib import Path
from urllib.parse import unquote, urlsplit

from .knowledge import GENERATED_END, GENERATED_START
from .paths import REQUIRED_FILES, index_dir, okf_dir


MARKDOWN_LINK = re.compile(r"(?<!!)\[[^\]]*\]\(([^)]+)\)")


def _generated_sections(text: str) -> tuple[list[str], str | None]:
    starts = [match.start() for match in re.finditer(re.escape(GENERATED_START), text)]
    ends = [match.start() for match in re.finditer(re.escape(GENERATED_END), text)]
    if not starts and not ends:
        return [], None
    if len(starts) != len(ends):
        return [], "generated markers are unpaired"
    sections: list[str] = []
    for start, end in zip(starts, ends):
        if start > end:
            return [], "generated markers are reversed"
        body_start = start + len(GENERATED_START)
        sections.append(text[body_start:end])
    return sections, None


def _validate_markdown(root: Path, document: Path, errors: list[str], *, validate_links: bool) -> None:
    text = document.read_text(encoding="utf-8", errors="ignore")
    relative_document = document.relative_to(root).as_posix()
    sections, marker_error = _generated_sections(text)
    if marker_error:
        errors.append(f"Malformed generated block in {relative_document}: {marker_error}")
        return
    if not validate_links:
        return
    for section in sections:
        for match in MARKDOWN_LINK.finditer(section):
            destination = match.group(1).strip().strip("<>")
            parsed = urlsplit(destination)
            if parsed.scheme or parsed.netloc or not parsed.path:
                continue
            target = (document.parent / unquote(parsed.path)).resolve()
            try:
                target.relative_to(root.resolve())
            except ValueError:
                errors.append(f"Generated link escapes repository in {relative_document}: {destination}")
                continue
            if not target.exists():
                errors.append(f"Broken generated link in {relative_document}: {destination}")


def validate(root: Path) -> dict[str, object]:
    errors: list[str] = []; warnings: list[str] = []
    for relative in REQUIRED_FILES:
        path = okf_dir(root) / relative
        if not path.exists(): errors.append(f"Missing required knowledge file: .okf/{relative}")
    manifest = okf_dir(root) / "manifest.yaml"
    if manifest.exists() and not re.search(r"^schema_version:\s*\d+", manifest.read_text(encoding="utf-8"), re.M): errors.append("manifest.yaml lacks schema_version")
    if not (index_dir(root) / "knowledge.sqlite3").exists(): warnings.append("Index is absent; run `okf-agent index`")
    for base, validate_links in ((okf_dir(root), False), (root / "docs" / "reference", True)):
        for doc in base.rglob("*.md") if base.exists() else []:
            _validate_markdown(root, doc, errors, validate_links=validate_links)
    return {"ok": not errors, "errors": errors, "warnings": warnings}
