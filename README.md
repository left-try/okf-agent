# OKF Repository Brain

`okf-agent` gives coding agents a repository-owned knowledge bundle in `.okf/` and a rebuildable local index in `.okf-index/`. It makes the same lifecycle available to Codex, Claude Code, Cursor, Gemini CLI, and any agent that follows repository instructions: retrieve context before a change, update knowledge after it, and validate before completion.

## Distribute It

Publish this repository to GitHub first. That is enough for users to install directly from its URL. Publishing to PyPI is optional, but makes the command shorter.

```powershell
# From a GitHub URL
pipx install "git+https://github.com/<owner>/okf-agent.git"

# After publishing the package to PyPI
pipx install okf-agent
```

To publish a release to PyPI, build a wheel and upload it from a tagged, tested commit:

```powershell
python -m pip install build twine
python -m build
python -m twine upload dist/*
```

## One-Time User Setup

After installing the CLI, users run this once inside each repository:

```powershell
okf-agent init .
```

It creates `.okf/`, builds the local index, installs Git hooks where they are safe to install, and writes native repository instructions for each supported client:

- `AGENTS.md` for Codex and generic agents
- `CLAUDE.md` and `.claude/rules/okf.md` for Claude Code
- `.cursor/rules/okf.mdc` for Cursor
- `GEMINI.md` for Gemini CLI

Existing instruction files are preserved and receive one OKF protocol block. Existing non-OKF Git hooks are left untouched.

For Codex users, the optional global installer activates OKF automatically for future repository tasks:

```powershell
okf-agent install
```

Restart Codex afterwards. For Claude Code, Cursor, and Gemini CLI, the project-native files created by `init` are loaded whenever the user opens that repository.

## Pasteable Agent Setup Prompt

Users can paste this into any terminal-capable coding agent after replacing the GitHub URL:

```text
Install OKF Repository Brain from git+https://github.com/<owner>/okf-agent.git using pipx, then run `okf-agent init .` in this repository. Do not overwrite existing agent instruction files or existing non-OKF Git hooks. After setup, run `okf-agent validate .` and report the generated adapter files.
```

Once initialized, the repository carries its own setup; collaborators only need the `okf-agent` CLI installed. `.okf/` is committed, while `.okf-index/` is local and can be deleted and rebuilt.
