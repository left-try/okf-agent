from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

from .discovery import build_index
from .docs import build_docs, check_docs
from .enforcement import install
from .knowledge import initialize
from .lifecycle import refresh_knowledge
from .mcp import create_server, context
from .onboarding import install_codex_skill
from .paths import find_root
from .validation import validate
from .watcher import start as start_watcher
from .web import serve_docs


def emit(value: object) -> None:
    print(json.dumps(value, indent=2, default=str))


def command_init(root: Path) -> dict:
    created = initialize(root)
    index = build_index(root)
    artifacts = install(root)
    docs = build_docs(root)
    return {
        "root": str(root),
        "created": [str(p.relative_to(root)) for p in created],
        "artifacts": [str(p.relative_to(root)) for p in artifacts],
        "index": index,
        "docs": docs,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="okf-agent")
    subs = parser.add_subparsers(dest="command", required=True)
    for name in ("init", "index", "update", "validate", "doctor"):
        p = subs.add_parser(name); p.add_argument("path", nargs="?", default=".")
        if name == "update": p.add_argument("--changed-files", nargs="*")
    install_parser = subs.add_parser("install", help="Install the automatic Codex skill for this user")
    install_parser.add_argument("--skills-dir", type=Path, help="Override the Codex skills directory")
    run = subs.add_parser("run", help="Prepare a repository for an agent and optionally launch its client"); run.add_argument("agent"); run.add_argument("path", nargs="?", default="."); run.add_argument("--no-launch", action="store_true")
    mcp = subs.add_parser("mcp"); mcp.add_argument("--repo", default=".")
    docs = subs.add_parser("docs", help="Build, check, or serve the local repository wiki")
    docs_subs = docs.add_subparsers(dest="docs_command", required=True)
    for name in ("build", "check"):
        docs_command = docs_subs.add_parser(name)
        docs_command.add_argument("path", nargs="?", default=".")
    serve = docs_subs.add_parser("serve")
    serve.add_argument("path", nargs="?", default=".")
    serve.add_argument("--host", default="127.0.0.1")
    serve.add_argument("--port", type=int, default=0)
    args = parser.parse_args(argv); root = find_root(getattr(args, "path", getattr(args, "repo", ".")))
    if args.command == "install":
        emit({"skill": str(install_codex_skill(args.skills_dir)), "next": "Restart Codex to discover the skill, then work normally in any repository."}); return 0
    if args.command == "init": emit(command_init(root)); return 0
    if args.command == "index": initialize(root); emit(build_index(root)); return 0
    if args.command == "update": initialize(root); emit(refresh_knowledge(root, args.changed_files)); return 0
    if args.command == "validate":
        result = validate(root); emit(result); return 0 if result["ok"] else 1
    if args.command == "doctor":
        result = {"root": str(root), "git": (root / ".git").exists(), "client": {a: bool(shutil.which(a)) for a in ("codex", "claude", "gemini")}, "validation": validate(root)}; emit(result); return 0 if result["validation"]["ok"] else 1
    if args.command == "run":
        initialize(root); build_index(root); install(root); build_docs(root)
        payload = {
            "agent": args.agent,
            "root": str(root),
            "context": context(root, "repository onboarding"),
            "instructions": "Call okf.get_change_context before editing, then update and validate.",
            "context_delivery": "Repository-native instructions carry the OKF protocol; this computed context is not injected into launched clients.",
        }
        if args.no_launch: emit(payload); return 0
        executable = {"cursor": "cursor", "windsurf": "windsurf", "claude-code": "claude", "gemini-cli": "gemini"}.get(args.agent, args.agent)
        if not shutil.which(executable): emit({**payload, "error": f"Client `{executable}` is not on PATH. Use --no-launch to prepare the repository."}); return 2
        emit(payload)
        watcher = start_watcher(root)
        try:
            return subprocess.run([executable], cwd=root).returncode
        finally:
            watcher.stop()
    if args.command == "mcp":
        try: create_server(args.repo).run()
        except RuntimeError as exc: parser.error(str(exc))
    if args.command == "docs":
        if args.docs_command == "build":
            emit(build_docs(root)); return 0
        if args.docs_command == "check":
            result = check_docs(root); emit(result); return 0 if result["ok"] else 1
        if args.docs_command == "serve":
            serve_docs(root, args.host, args.port); return 0
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
