from __future__ import annotations

import shutil
import subprocess
from dataclasses import dataclass


@dataclass(frozen=True)
class Plugin:
    marketplace: str
    package: str
    repository: str


PROFILES: dict[str, tuple[Plugin, ...]] = {
    "default": (
        Plugin("mattpocock", "mattpocock-skills", "mattpocock/skills"),
        Plugin("ponytail", "ponytail", "DietrichGebert/ponytail"),
    ),
}


def apply_profile(profile: str, agent: str = "codex") -> dict:
    """Install a named set of native plugins using the agent's own CLI."""
    if profile not in PROFILES:
        raise ValueError(f"Unknown skills profile: {profile}")
    if agent != "codex":
        raise ValueError("The built-in profiles currently support only the Codex plugin CLI.")
    executable = shutil.which("codex")
    if not executable:
        raise RuntimeError("Codex CLI is not on PATH; install Codex or use `okf-agent skills doctor`.")

    actions: list[dict[str, str]] = []
    for plugin in PROFILES[profile]:
        commands = (
            [executable, "plugin", "marketplace", "add", plugin.repository],
            [executable, "plugin", "add", f"{plugin.package}@{plugin.marketplace}"],
        )
        for command in commands:
            completed = subprocess.run(command, capture_output=True, text=True, check=False)
            actions.append({
                "command": " ".join(command[1:]),
                "status": "ok" if completed.returncode == 0 else "failed",
                "output": completed.stdout.strip(),
                "error": completed.stderr.strip() or None,
            })
            if completed.returncode:
                raise RuntimeError(f"Codex command failed ({completed.returncode}): {' '.join(command[1:])}")
    return {
        "profile": profile,
        "agent": agent,
        "installed": [plugin.package for plugin in PROFILES[profile]],
        "actions": actions,
        "next": "Restart Codex or start a new task to load the installed plugins. Review and trust Ponytail's lifecycle hooks in Codex before enabling them.",
    }


def doctor(agent: str = "codex") -> dict:
    if agent != "codex":
        raise ValueError("The built-in profiles currently support only the Codex plugin CLI.")
    executable = shutil.which("codex")
    if not executable:
        return {"agent": agent, "available": False, "error": "Codex CLI is not on PATH."}

    result: dict = {"agent": agent, "available": True}
    for name, args in (
        ("marketplaces", ["plugin", "marketplace", "list"]),
        ("plugins", ["plugin", "list"]),
    ):
        completed = subprocess.run([executable, *args], capture_output=True, text=True, check=False)
        result[name] = {
            "ok": completed.returncode == 0,
            "output": completed.stdout.strip(),
            "error": completed.stderr.strip() or None,
        }
    result["ok"] = all(item["ok"] for key, item in result.items() if key in ("marketplaces", "plugins"))
    return result
