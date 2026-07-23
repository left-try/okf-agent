from __future__ import annotations

from pathlib import Path


REQUIRED_FILES = {
    "manifest.yaml": "schema_version: 1\nname: {name}\n",
    "index.md": "# Repository Knowledge\n\n<!-- okf:generated:start -->\n<!-- okf:generated:end -->\n",
    "application/overview.md": "# Application Overview\n\n<!-- okf:generated:start -->\n<!-- okf:generated:end -->\n\n## Curated intent\nDescribe the product purpose here.\n",
    "architecture/index.md": "# Architecture\n\n<!-- okf:generated:start -->\n<!-- okf:generated:end -->\n\n## Curated constraints\nRecord architectural constraints here.\n",
    "operations/development.md": "# Development\n\n<!-- okf:generated:start -->\n<!-- okf:generated:end -->\n",
}


def find_root(path: str | Path) -> Path:
    candidate = Path(path).expanduser().resolve()
    candidate = candidate if candidate.is_dir() else candidate.parent
    for parent in (candidate, *candidate.parents):
        if (parent / ".git").exists():
            return parent
    return candidate


def okf_dir(root: Path) -> Path:
    return root / ".okf"


def index_dir(root: Path) -> Path:
    return root / ".okf-index"
