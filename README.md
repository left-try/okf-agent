# OKF Repository Brain

`okf-agent` gives coding agents a repository-owned knowledge bundle in `.okf/` and a rebuildable local index in `.okf-index/`. The lifecycle is automatic: retrieve context before a change, update knowledge after it, and validate before completion.

## Install for your agents

Install the package, then install the Codex skill once for the current user:

```powershell
pipx install okf-agent
okf-agent install
```

For a checkout of this repository before publishing a package:

```powershell
pipx install .
okf-agent install
```

Restart Codex after installation. From then on, the skill applies to repository work without users having to mention OKF. On an agent's first task in a repository it initializes `.okf/`, generates `AGENTS.md`, and builds the local index. Git hooks keep the lifecycle active outside agent sessions.

## Per-repository setup

The skill runs this automatically, but users can also opt in explicitly:

```powershell
okf-agent init C:\path\to\repository
okf-agent index C:\path\to\repository
okf-agent doctor C:\path\to\repository
```

`.okf/` is committed. `.okf-index/` is local and can be deleted and rebuilt at any time.
