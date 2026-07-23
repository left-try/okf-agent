from __future__ import annotations

import subprocess
from datetime import UTC, datetime
from pathlib import Path

from .discovery import build_index, detect
from .knowledge import replace_generated
from .paths import okf_dir


def git_changed(root: Path) -> list[str]:
    result = subprocess.run(["git", "diff", "--name-only", "HEAD"], cwd=root, capture_output=True, text=True)
    return [line for line in result.stdout.splitlines() if line] if result.returncode == 0 else []


def refresh_knowledge(root: Path, changed: list[str] | None = None) -> dict[str, object]:
    facts = detect(root)
    changed = changed if changed is not None else git_changed(root)
    stamp = datetime.now(UTC).isoformat()
    replace_generated(okf_dir(root) / "application/overview.md", "## Generated facts\n- Languages: " + (", ".join(facts["languages"]) or "unknown") + "\n- Framework hints: " + (", ".join(facts["framework_hints"]) or "none"))
    replace_generated(okf_dir(root) / "architecture/index.md", "## Generated facts\n- Manifests: " + (", ".join(facts["manifests"]) or "none") + "\n- Last refreshed: " + stamp)
    replace_generated(okf_dir(root) / "changes" / "current.md", "# Current change context\n\n<!-- okf:generated:start -->\nUpdated: " + stamp + "\n\nChanged files:\n" + ("\n".join(f"- `{p}`" for p in changed) or "- No Git diff detected") + "\n<!-- okf:generated:end -->\n")
    return {"changed_files": changed, "facts": facts}
