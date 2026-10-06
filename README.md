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

It creates `.okf/`, builds the local index and local wiki, installs Git hooks where they are safe to install, and writes native repository instructions for each supported client:

- `AGENTS.md` for Codex and generic agents
- `CLAUDE.md` and `.claude/rules/okf.md` for Claude Code
- `.cursor/rules/okf.mdc` for Cursor
- `GEMINI.md` for Gemini CLI

Existing instruction files are preserved and receive one OKF protocol block. Existing non-OKF Git hooks are left untouched.

The wiki is generated into `.okf-index/site/`, which is local and rebuildable. It has a page list, project facts, copies of `.okf/` Markdown pages, and simple text search. Start the read-only local web UI with:

```powershell
okf-agent docs serve .
```

It binds to `127.0.0.1` by default. To rebuild or check the generated pages explicitly:

```powershell
okf-agent docs build .
okf-agent docs check .
```

`okf-agent update .` refreshes repository facts and the SQLite index. If the local wiki has already been created, `update` rebuilds it too. `okf-agent validate .` checks for stale wiki output, and the generated pre-push hook runs validation. A stale or corrupted generated site makes validation fail; rebuild it with `okf-agent docs build .`.

The generated site is not published or pushed to GitHub. `.okf/` remains the source of truth. The site includes deterministic project facts and mirrors curated Markdown pages; it does not invent architectural explanations.

The source index currently scans `.py`, `.js`, `.ts`, `.tsx`, `.jsx`, `.go`, `.rs`, `.java`, `.cs`, `.rb`, and `.php` files. It also indexes `pyproject.toml`, `requirements.txt`, `package.json`, `go.mod`, `Cargo.toml`, `pom.xml`, `Dockerfile`, `docker-compose.yml`, and `.env.example`. Language facts are inferred from Python, Node.js, Go, Rust, and Maven manifests; the current framework hint is Node.js for `package.json`.

For Codex users, the optional global installer activates OKF automatically for future repository tasks:

```powershell
okf-agent install
```

Restart Codex afterwards. For Claude Code, Cursor, and Gemini CLI, the project-native files created by `init` are loaded whenever the user opens that repository.

`okf-agent run <agent>` prepares the repository and can launch the named client. It writes the project-native instructions, but the computed context in its JSON payload is not injected into the launched client. Configure the optional MCP server in a client to expose context retrieval and update tools directly.

## Adaptive Development Workflows

Workflow profiles are optional. Install the starter SDD, TDD, and GTDD policy when initializing a repository:

~~~powershell
okf-agent init . --profiles sdd,tdd,gtdd
~~~

Initialization preserves existing curated workflow files. Without the profiles option, existing initialization behavior is unchanged and no workflow configuration is installed. The optional configuration lives in .okf/workflows/config.json; .okf/workflows/active.md explains how agents select only relevant profile and role guidance.

Classify changes by intent as feature, bugfix, hotfix, docs, tests, refactor, or maintenance. Assess rigor independently as light, standard, or strict, based on ambiguity, affected components, statefulness, security/data impact, reversibility, production exposure, and test confidence. Use --high-impact to force strict rigor for security, data, money, or production risk. Typical starting points are SDD + TDD for features, regression-first TDD for bug fixes, and light factual/link checks for prose-only docs. Strict work adds an explicit behavior contract and broader verification. Urgency never waives critical checks.

Inspect policy or resolve an explicit assessment without changing repository state:

~~~powershell
okf-agent workflow show .
okf-agent workflow explain . --type bugfix --risk standard
okf-agent workflow resolve . --type feature --risk strict --stateful --execution single-agent
~~~

CLI and MCP use the same resolver. The task assessment is explicit; OKF does not infer arbitrary task semantics from prose. Independent GTDD requires separate coder, tester, and auditor contexts plus controlled handoffs. Until an executor provides those capabilities, resolution reports a single-agent adversarial review fallback. Installing GTDD profile documents does not create independent execution. OKF knowledge validation checks repository knowledge integrity and does not prove application tests passed.

## Pasteable Agent Setup Prompt

Users can paste this into any terminal-capable coding agent after replacing the GitHub URL:

```text
Install OKF Repository Brain from git+https://github.com/<owner>/okf-agent.git using pipx, then run `okf-agent init .` in this repository. Do not overwrite existing agent instruction files or existing non-OKF Git hooks. After setup, run `okf-agent validate .` and report the generated adapter files.
```

Once initialized, the repository carries its own setup; collaborators only need the `okf-agent` CLI installed. `.okf/` is committed, while `.okf-index/` is local and can be deleted and rebuilt.
