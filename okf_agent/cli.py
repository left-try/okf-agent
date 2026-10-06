from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

from .discovery import build_index
from .docs import build_docs, check_docs
from .enforcement import install_report
from .knowledge import initialize
from .lifecycle import refresh_knowledge
from .mcp import create_server, context
from .onboarding import install_codex_skill
from .paths import find_root
from .validation import validate
from .workflows import (
    WorkflowAssessment,
    WorkflowCapabilities,
    WorkflowConfigError,
    install_profiles,
    load_policy,
    resolve_workflow,
    workflow_data,
)
from .watcher import start as start_watcher
from .web import serve_docs


def emit(value: object) -> None:
    print(json.dumps(value, indent=2, default=str))


def command_init(root: Path, profiles: tuple[str, ...] | None = None) -> dict:
    created = initialize(root)
    profile_result = install_profiles(root, profiles) if profiles else None
    if profile_result:
        created.extend(root / path for path in (*profile_result.created, *profile_result.updated))
    index = build_index(root)
    installation = install_report(root, workflow_profiles=bool(profiles) if profiles else None)
    artifacts = list(installation.installed)
    docs = build_docs(root)
    result = {
        "root": str(root),
        "created": [str(p.relative_to(root)) for p in created],
        "artifacts": [str(p.relative_to(root)) for p in artifacts],
        "index": index,
        "docs": docs,
    }
    if profile_result:
        result["workflow_profiles"] = {
            "enabled_profiles": list(profiles or ()),
            "created": list(profile_result.created),
            "retained": list(profile_result.retained),
            "updated": list(profile_result.updated),
            "conflicts": list(profile_result.conflicts),
        }
    if installation.conflicts:
        result["adapter_conflicts"] = list(installation.conflicts)
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="okf-agent")
    subs = parser.add_subparsers(dest="command", required=True)
    for name in ("init", "index", "update", "validate", "doctor"):
        p = subs.add_parser(name); p.add_argument("path", nargs="?", default=".")
        if name == "update":
            p.add_argument("--changed-files", nargs="*")
        elif name == "init":
            p.add_argument("--profiles", help="Opt in to comma-separated workflow profiles (sdd,tdd,gtdd)")
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
    workflow = subs.add_parser("workflow", help="Inspect or resolve repository workflow policy")
    workflow_subs = workflow.add_subparsers(dest="workflow_command", required=True)
    workflow_subs.add_parser("show", help="Show validated repository workflow policy").add_argument("path", nargs="?", default=".")
    for command in ("explain", "resolve"):
        command_parser = workflow_subs.add_parser(command)
        command_parser.add_argument("path", nargs="?", default=".")
        command_parser.add_argument("--type", required=True, choices=("feature", "bugfix", "hotfix", "docs", "tests", "refactor", "maintenance"))
        command_parser.add_argument("--secondary-type", action="append", choices=("feature", "bugfix", "hotfix", "docs", "tests", "refactor", "maintenance"), default=[])
        command_parser.add_argument("--risk", choices=("light", "standard", "strict"))
        command_parser.add_argument("--method", action="append", choices=("sdd", "tdd", "gtdd"), default=[])
        command_parser.add_argument("--ambiguous", action="store_true")
        command_parser.add_argument("--behavioral", action="store_true")
        command_parser.add_argument("--high-impact", action="store_true", help="Force strict rigor for security, data, money, or production risk")
        command_parser.add_argument("--stateful", action="store_true", help="Mark independent challenge as materially useful")
        command_parser.add_argument("--execution", choices=("single-agent", "independent-roles"), default="single-agent")
    args = parser.parse_args(argv); root = find_root(getattr(args, "path", getattr(args, "repo", ".")))
    if args.command == "install":
        emit({"skill": str(install_codex_skill(args.skills_dir)), "next": "Restart Codex to discover the skill, then work normally in any repository."}); return 0
    if args.command == "init":
        profiles = tuple(part.strip() for part in args.profiles.split(",") if part.strip()) if args.profiles else None
        try:
            emit(command_init(root, profiles))
        except (ValueError, WorkflowConfigError) as exc:
            emit({"error": str(exc)})
            return 2
        return 0
    if args.command == "index": initialize(root); emit(build_index(root)); return 0
    if args.command == "update": initialize(root); emit(refresh_knowledge(root, args.changed_files)); return 0
    if args.command == "validate":
        result = validate(root); emit(result); return 0 if result["ok"] else 1
    if args.command == "doctor":
        result = {"root": str(root), "git": (root / ".git").exists(), "client": {a: bool(shutil.which(a)) for a in ("codex", "claude", "gemini")}, "validation": validate(root)}; emit(result); return 0 if result["validation"]["ok"] else 1
    if args.command == "run":
        initialize(root); build_index(root)
        installation = install_report(root)
        build_docs(root)
        payload = {
            "agent": args.agent,
            "root": str(root),
            "context": context(root, "repository onboarding"),
            "instructions": "Call okf.get_change_context before editing, then update and validate.",
            "context_delivery": "Repository-native instructions carry the OKF protocol; this computed context is not injected into launched clients.",
        }
        if installation.conflicts:
            payload["workflow_adapter_conflicts"] = list(installation.conflicts)
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
    if args.command == "workflow":
        try:
            policy = load_policy(root)
            if args.workflow_command == "show":
                emit(workflow_data(policy))
                return 0
            assessment = WorkflowAssessment(
                primary_type=args.type,
                secondary_types=tuple(args.secondary_type),
                risk=args.risk,
                requested_methods=tuple(args.method),
                ambiguous=args.ambiguous,
                behavioral=args.behavioral,
                independent_challenge=args.stateful,
                high_impact=args.high_impact,
            )
            capabilities = WorkflowCapabilities(
                independent_roles=args.execution == "independent-roles",
                enforceable_handoffs=args.execution == "independent-roles",
            )
            emit(workflow_data(resolve_workflow(policy, assessment, capabilities)))
            return 0
        except (ValueError, WorkflowConfigError) as exc:
            emit({"error": str(exc)})
            return 2
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
