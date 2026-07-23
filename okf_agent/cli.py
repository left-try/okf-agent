from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

from .discovery import build_index
from .enforcement import install
from .knowledge import initialize
from .lifecycle import refresh_knowledge
from .mcp import create_server, context
from .paths import find_root
from .validation import validate
from .watcher import start as start_watcher


def emit(value: object) -> None:
    print(json.dumps(value, indent=2, default=str))


def command_init(root: Path) -> dict:
    return {"root": str(root), "created": [str(p.relative_to(root)) for p in initialize(root)], "artifacts": [str(p.relative_to(root)) for p in install(root)]}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="okf-agent")
    subs = parser.add_subparsers(dest="command", required=True)
    for name in ("init", "index", "update", "validate", "doctor"):
        p = subs.add_parser(name); p.add_argument("path", nargs="?", default=".")
        if name == "update": p.add_argument("--changed-files", nargs="*")
    run = subs.add_parser("run"); run.add_argument("agent"); run.add_argument("path", nargs="?", default="."); run.add_argument("--no-launch", action="store_true")
    mcp = subs.add_parser("mcp"); mcp.add_argument("--repo", default=".")
    args = parser.parse_args(argv); root = find_root(getattr(args, "path", getattr(args, "repo", ".")))
    if args.command == "init": emit(command_init(root)); return 0
    if args.command == "index": initialize(root); emit(build_index(root)); return 0
    if args.command == "update": initialize(root); emit(refresh_knowledge(root, args.changed_files)); return 0
    if args.command == "validate":
        result = validate(root); emit(result); return 0 if result["ok"] else 1
    if args.command == "doctor":
        result = {"root": str(root), "git": (root / ".git").exists(), "client": {a: bool(shutil.which(a)) for a in ("codex", "claude", "gemini")}, "validation": validate(root)}; emit(result); return 0 if result["validation"]["ok"] else 1
    if args.command == "run":
        initialize(root); build_index(root); install(root); start_watcher(root)
        payload = {"agent": args.agent, "root": str(root), "context": context(root, "repository onboarding"), "instructions": "Call okf.get_change_context before editing, then update and validate."}
        if args.no_launch: emit(payload); return 0
        executable = {"cursor": "cursor", "windsurf": "windsurf", "claude-code": "claude", "gemini-cli": "gemini"}.get(args.agent, args.agent)
        if not shutil.which(executable): emit({**payload, "error": f"Client `{executable}` is not on PATH. Use --no-launch to prepare the repository."}); return 2
        return subprocess.run([executable], cwd=root).returncode
    if args.command == "mcp":
        try: create_server(args.repo).run()
        except RuntimeError as exc: parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
