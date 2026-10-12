from __future__ import annotations

import subprocess
from pathlib import Path

from .discovery import build_index, detect
from .docs import build_docs
from .knowledge import replace_generated
from .paths import index_dir, okf_dir


def git_changed(root: Path) -> list[str]:
    changed: set[str] = set()
    for command in (
        ["git", "diff", "--name-only", "HEAD", "--"],
        ["git", "diff", "--cached", "--name-only", "--"],
        ["git", "ls-files", "--others", "--exclude-standard", "--"],
    ):
        result = subprocess.run(command, cwd=root, capture_output=True, text=True)
        if result.returncode == 0:
            changed.update(Path(line).as_posix() for line in result.stdout.splitlines() if line)
    return sorted(changed)


def refresh_knowledge(root: Path, changed: list[str] | None = None) -> dict[str, object]:
    facts = detect(root)
    changed = changed if changed is not None else git_changed(root)
    replace_generated(okf_dir(root) / "application/overview.md", "## Generated facts\n- Languages: " + (", ".join(facts["languages"]) or "unknown") + "\n- Framework hints: " + (", ".join(facts["framework_hints"]) or "none"))
    replace_generated(okf_dir(root) / "architecture/index.md", "## Generated facts\n- Manifests: " + (", ".join(facts["manifests"]) or "none"))
    current_doc = okf_dir(root) / "changes" / "current.md"
    if not current_doc.exists():
        current_doc.parent.mkdir(parents=True, exist_ok=True)
        current_doc.write_text(
            "# Current change context\n\n"
            "<!-- okf:generated:start -->\n<!-- okf:generated:end -->\n",
            encoding="utf-8",
        )
    replace_generated(
        current_doc,
        "Changed files:\n"
        + ("\n".join(f"- `{p}`" for p in changed) or "- No Git diff detected"),
    )
    index = build_index(root)
    site = index_dir(root) / "site"
    docs = build_docs(root) if site.exists() else None
    return {"changed_files": changed, "facts": facts, "index": index, "docs": docs}
