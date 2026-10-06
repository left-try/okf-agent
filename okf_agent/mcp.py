from __future__ import annotations

from pathlib import Path

from .discovery import build_index, search
from .knowledge import initialize, record_decision
from .lifecycle import refresh_knowledge
from .paths import find_root, okf_dir
from .validation import validate
from .workflows import WorkflowAssessment, WorkflowCapabilities, load_policy, resolve_workflow, workflow_data


def context(root: Path, request: str) -> dict[str, object]:
    results = search(root, request)
    concepts = []
    for doc in okf_dir(root).rglob("*.md") if okf_dir(root).exists() else []:
        if request.lower() in doc.read_text(encoding="utf-8", errors="ignore").lower(): concepts.append(doc.relative_to(root).as_posix())
    result = {"request": request, "matches": results, "concepts": concepts[:8], "protocol": "Retrieve context before edits; update knowledge after edits; validate before completion."}
    if (okf_dir(root) / "workflows" / "config.json").is_file() and (okf_dir(root) / "workflows" / "active.md").is_file():
        result["workflow_routing"] = (
            "Read .okf/workflows/active.md, classify the task and assess risk, "
            "then load only selected profiles and assigned roles."
        )
    return result


def create_server(repo: str | Path = "."):
    """Return a FastMCP server when the optional dependency is installed."""
    try:
        from fastmcp import FastMCP
    except ImportError as exc:
        raise RuntimeError("MCP support requires `pip install okf-agent[mcp]`.") from exc
    root = find_root(repo)
    server = FastMCP("OKF Repository Brain")

    @server.tool(name="okf.initialize")
    def initialize_tool() -> dict: return {"created": [str(p.relative_to(root)) for p in initialize(root)]}
    @server.tool(name="okf.status")
    def status() -> dict: return {"root": str(root), "validation": validate(root)}
    @server.tool(name="okf.search")
    def search_tool(query: str, limit: int = 8) -> list[dict]: return search(root, query, limit)
    @server.tool(name="okf.get_concept")
    def get_concept(path: str) -> str:
        target = (okf_dir(root) / path).resolve()
        bundle = okf_dir(root).resolve()
        if bundle not in target.parents or target.suffix != ".md":
            raise ValueError("Concept path must be a Markdown file inside .okf/")
        return target.read_text(encoding="utf-8")
    @server.tool(name="okf.get_change_context")
    def get_change_context(request: str) -> dict: return context(root, request)
    @server.tool(name="okf.get_workflow_policy")
    def get_workflow_policy() -> dict: return workflow_data(load_policy(root))
    @server.tool(name="okf.resolve_workflow")
    def resolve_workflow_tool(
        primary_type: str,
        secondary_types: list[str] | None = None,
        risk: str | None = None,
        requested_methods: list[str] | None = None,
        ambiguous: bool = False,
        behavioral: bool = False,
        stateful: bool = False,
        high_impact: bool = False,
        independent_roles: bool = False,
        enforceable_handoffs: bool = False,
    ) -> dict:
        assessment = WorkflowAssessment(
            primary_type=primary_type,
            secondary_types=tuple(secondary_types or ()),
            risk=risk,
            requested_methods=tuple(requested_methods or ()),
            ambiguous=ambiguous,
            behavioral=behavioral,
            independent_challenge=stateful,
            high_impact=high_impact,
        )
        capabilities = WorkflowCapabilities(independent_roles, enforceable_handoffs)
        return workflow_data(resolve_workflow(load_policy(root), assessment, capabilities))
    @server.tool(name="okf.record_decision")
    def record_decision_tool(title: str, rationale: str, sources: list[str] | None = None) -> dict: return {"path": str(record_decision(root, title, rationale, sources).relative_to(root))}
    @server.tool(name="okf.update_after_change")
    def update_after_change(changed_files: list[str] | None = None) -> dict: return refresh_knowledge(root, changed_files)
    @server.tool(name="okf.validate")
    def validate_tool() -> dict: return validate(root)
    @server.tool(name="okf.reindex")
    def reindex() -> dict: return build_index(root)
    return server
