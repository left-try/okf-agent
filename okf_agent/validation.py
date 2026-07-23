from __future__ import annotations

import re
from pathlib import Path

from .paths import REQUIRED_FILES, index_dir, okf_dir


def validate(root: Path) -> dict[str, object]:
    errors: list[str] = []; warnings: list[str] = []
    for relative in REQUIRED_FILES:
        path = okf_dir(root) / relative
        if not path.exists(): errors.append(f"Missing required knowledge file: .okf/{relative}")
    manifest = okf_dir(root) / "manifest.yaml"
    if manifest.exists() and not re.search(r"^schema_version:\s*\d+", manifest.read_text(encoding="utf-8"), re.M): errors.append("manifest.yaml lacks schema_version")
    if not (index_dir(root) / "knowledge.sqlite3").exists(): warnings.append("Index is absent; run `okf-agent index`")
    for doc in okf_dir(root).rglob("*.md") if okf_dir(root).exists() else []:
        text = doc.read_text(encoding="utf-8", errors="ignore")
        if "<!-- okf:generated:start -->" in text and "<!-- okf:generated:end -->" not in text: errors.append(f"Unclosed generated block: {doc.relative_to(root)}")
    return {"ok": not errors, "errors": errors, "warnings": warnings}
