from __future__ import annotations

import sys
from dataclasses import dataclass
from pathlib import Path


PROTOCOL = """## OKF Repository Protocol

Before planning or editing, run `okf-agent index .` when the index is absent, then retrieve repository context. After source changes, run `okf-agent update . --changed-files <changed paths>`. Before completion, run `okf-agent validate .`. Preserve curated content outside OKF-generated blocks.
"""

CURSOR_RULE = """---
description: Maintain OKF repository knowledge before and after every coding task.
alwaysApply: true
---

""" + PROTOCOL

WORKFLOW_START = "<!-- okf:workflow:start -->"
WORKFLOW_END = "<!-- okf:workflow:end -->"
WORKFLOW_ROUTING = f"""{WORKFLOW_START}
## OKF Workflow Routing

Read `.okf/workflows/active.md` when present. Classify the task and assess risk, then load only the selected workflow profile and assigned role instructions. Use independent GTDD only when separate role contexts and controlled handoffs are available; otherwise report a single-agent adversarial review.
{WORKFLOW_END}"""

HOOKS = {
    "post-checkout": "okf-agent index . || exit 0\n",
    "post-merge": "okf-agent index . || exit 0\n",
    "pre-commit": "okf-agent update .\n",
    "pre-push": "okf-agent validate .\n",
}


@dataclass(frozen=True)
class InstallReport:
    installed: tuple[Path, ...] = ()
    conflicts: tuple[str, ...] = ()


def _command() -> str:
    executable = Path(sys.executable).parent / "Scripts" / "okf-agent.exe"
    return str(executable) if executable.exists() else "okf-agent"


def _append_protocol(path: Path) -> bool:
    if path.exists():
        current = path.read_text(encoding="utf-8")
        if "## OKF Repository Protocol" in current:
            return False
        path.write_text(current.rstrip() + "\n\n" + PROTOCOL, encoding="utf-8")
        return True
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("# Repository Instructions\n\n" + PROTOCOL, encoding="utf-8")
    return True


def _install_workflow_routing(path: Path) -> tuple[bool, str | None]:
    current = path.read_text(encoding="utf-8")
    starts = current.count(WORKFLOW_START)
    ends = current.count(WORKFLOW_END)
    if starts != ends or starts > 1:
        return False, f"{path.name}: malformed or duplicate workflow routing markers"
    if starts == 1:
        start = current.index(WORKFLOW_START)
        end = current.index(WORKFLOW_END) + len(WORKFLOW_END)
        replacement = current[:start] + WORKFLOW_ROUTING + current[end:]
    else:
        replacement = current.rstrip() + "\n\n" + WORKFLOW_ROUTING + "\n"
    if replacement == current:
        return False, None
    path.write_text(replacement, encoding="utf-8")
    return True, None


def _adapter_is_within_root(root: Path, target: Path) -> bool:
    try:
        target.resolve(strict=False).relative_to(root.resolve())
    except (OSError, ValueError):
        return False
    return True


def install_report(root: Path, workflow_profiles: bool | None = None) -> InstallReport:
    installed: list[Path] = []
    conflicts: list[str] = []
    adapters = [root / "AGENTS.md", root / "CLAUDE.md", root / "GEMINI.md", root / ".claude" / "rules" / "okf.md"]
    safe_adapters: list[Path] = []
    for target in adapters:
        if not _adapter_is_within_root(root, target):
            conflicts.append(f"{target.relative_to(root)}: unsafe path outside repository preserved")
            continue
        if target.is_symlink():
            conflicts.append(f"{target.relative_to(root)}: unsafe symlink preserved")
            continue
        safe_adapters.append(target)
        if _append_protocol(target):
            installed.append(target)
    cursor_rule = root / ".cursor" / "rules" / "okf.mdc"
    if not _adapter_is_within_root(root, cursor_rule):
        conflicts.append(f"{cursor_rule.relative_to(root)}: unsafe path outside repository preserved")
    elif cursor_rule.is_symlink():
        conflicts.append(f"{cursor_rule.relative_to(root)}: unsafe symlink preserved")
    elif not cursor_rule.exists():
        cursor_rule.parent.mkdir(parents=True, exist_ok=True)
        cursor_rule.write_text(CURSOR_RULE, encoding="utf-8")
        installed.append(cursor_rule)
        safe_adapters.append(cursor_rule)
    else:
        safe_adapters.append(cursor_rule)

    has_workflow = workflow_profiles
    if has_workflow is None:
        has_workflow = (root / ".okf" / "workflows" / "config.json").is_file()
    if has_workflow:
        for target in safe_adapters:
            try:
                changed, conflict = _install_workflow_routing(target)
            except (OSError, UnicodeError) as exc:
                conflicts.append(f"{target.relative_to(root)}: cannot install workflow routing: {exc}")
                continue
            if conflict:
                conflicts.append(f"{target.relative_to(root)}: {conflict}")
            elif changed:
                installed.append(target)

    hooks = root / ".git" / "hooks"
    if hooks.exists():
        for name, command in HOOKS.items():
            target = hooks / name
            if target.exists() and "# Generated by okf-agent" not in target.read_text(encoding="utf-8", errors="ignore"):
                continue
            try:
                target.write_text(
                    "#!/bin/sh\n# Generated by okf-agent\n" + command.replace("okf-agent", f"\"{_command()}\""),
                    encoding="utf-8",
                )
                target.chmod(0o755)
                installed.append(target)
            except OSError:
                continue
    return InstallReport(tuple(dict.fromkeys(installed)), tuple(conflicts))


def install(root: Path) -> list[Path]:
    """Install native OKF instructions and safe hooks (legacy return type)."""
    return list(install_report(root).installed)
