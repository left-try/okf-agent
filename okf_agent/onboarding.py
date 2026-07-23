from __future__ import annotations

import os
from pathlib import Path


SKILL_NAME = "okf-repository-brain"
SKILL_CONTENT = """---
name: okf-repository-brain
description: Automatically initialize and maintain the OKF Repository Brain for every repository-based coding task. Use whenever inspecting, planning, changing, testing, reviewing, or debugging code in a local repository, even when the user does not mention OKF.
---

Run the OKF lifecycle automatically for every code repository task.

At the start of work, resolve the repository root. Run `okf-agent init <root>` when `.okf/` is absent, then run `okf-agent index <root>` when the local index is absent or stale. Read `AGENTS.md` and retrieve OKF context before planning or editing.

After source changes and before the final response, run `okf-agent update <root> --changed-files <changed paths>` followed by `okf-agent validate <root>`. Fix validation failures caused by the current work. Preserve curated content outside generated OKF blocks.

Do this without asking the user to invoke OKF. If the command is unavailable, state that the OKF installation is incomplete; do not silently skip the lifecycle.
"""


def default_codex_skills_dir() -> Path:
    codex_home = Path(os.environ.get("CODEX_HOME", Path.home() / ".codex"))
    return codex_home / "skills"


def install_codex_skill(skills_dir: Path | None = None) -> Path:
    target = (skills_dir or default_codex_skills_dir()) / SKILL_NAME
    target.mkdir(parents=True, exist_ok=True)
    (target / "SKILL.md").write_text(SKILL_CONTENT, encoding="utf-8")
    return target
