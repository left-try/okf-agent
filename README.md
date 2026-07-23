# OKF Repository Brain

`okf-agent` keeps a small, repository-owned knowledge bundle in `.okf/` and a rebuildable local index in `.okf-index/`. It gives coding agents one lifecycle: retrieve context before a change, update knowledge after it, and validate before completion.

## Quick start

```powershell
pip install -e .
okf-agent init C:\path\to\repository
okf-agent index C:\path\to\repository
okf-agent doctor C:\path\to\repository
okf-agent mcp --repo C:\path\to\repository
```

Use `okf-agent update --changed-files src/api.py` after an edit, or let the generated Git hooks flag a missing update. `okf-agent run codex` prepares the index and prints the adapter instructions before delegating to an installed client.

`.okf/` is committed; `.okf-index/` is intentionally local and can always be rebuilt.
