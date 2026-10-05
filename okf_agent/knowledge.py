from __future__ import annotations

import re
from datetime import UTC, datetime
from pathlib import Path

from .paths import REQUIRED_FILES, index_dir, okf_dir


GENERATED_START = "<!-- okf:generated:start -->"
GENERATED_END = "<!-- okf:generated:end -->"


def initialize(root: Path) -> list[Path]:
    created: list[Path] = []
    bundle = okf_dir(root)
    for relative, template in REQUIRED_FILES.items():
        target = bundle / relative
        if not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(template.format(name=root.name), encoding="utf-8")
            created.append(target)
    for folder in ("decisions", "changes"):
        (bundle / folder).mkdir(parents=True, exist_ok=True)
    index_dir(root).mkdir(parents=True, exist_ok=True)
    return created


def replace_generated(document: Path, body: str) -> None:
    if not document.exists():
        document.parent.mkdir(parents=True, exist_ok=True)
        title = document.stem.replace("-", " ").title()
        document.write_text(
            f"# {title}\n\n{GENERATED_START}\n{GENERATED_END}\n",
            encoding="utf-8",
        )
    text = document.read_text(encoding="utf-8")
    replacement = f"{GENERATED_START}\n{body.rstrip()}\n{GENERATED_END}"
    marker = re.compile(re.escape(GENERATED_START) + "|" + re.escape(GENERATED_END))
    matches = list(marker.finditer(text))
    if not matches:
        text += "\n\n" + replacement + "\n"
    else:
        pieces: list[str] = []
        cursor = 0
        active = False
        for match in matches:
            token = match.group(0)
            if token == GENERATED_START:
                if not active:
                    pieces.append(text[cursor:match.start()])
                    pieces.append(f"{GENERATED_START}\n{body.rstrip()}\n")
                    active = True
            elif active:
                # Drop the previous generated body, keep only curated text
                # outside the machine-owned markers.
                pieces.append(GENERATED_END)
                active = False
            else:
                pieces.append(text[cursor:match.start()])
            cursor = match.end()
        if not active:
            pieces.append(text[cursor:])
        if active:
            pieces.append(GENERATED_END)
        text = "".join(pieces)
    if text != document.read_text(encoding="utf-8"):
        document.write_text(text, encoding="utf-8")


def record_decision(root: Path, title: str, rationale: str, sources: list[str] | None = None) -> Path:
    slug = re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-") or "decision"
    stamp = datetime.now(UTC).strftime("%Y%m%d-%H%M%S")
    target = okf_dir(root) / "decisions" / f"{stamp}-{slug}.md"
    refs = "\n".join(f"- `{item}`" for item in sources or []) or "- None recorded"
    target.write_text(f"# {title}\n\n## Rationale\n{rationale}\n\n## Source references\n{refs}\n", encoding="utf-8")
    return target
