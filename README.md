# OKF Repository Brain

`okf-agent` gives coding agents a repository-owned knowledge bundle in `.okf/` and a rebuildable local index in `.okf-index/`. It makes the same lifecycle available to Codex, Claude Code, Cursor, Gemini CLI, and any agent that follows repository instructions: retrieve context before a change, update knowledge after it, and validate before completion.

## Distribute It

Publish this repository to GitHub first. That is enough for users to install directly from its URL. Publishing to PyPI is optional, but makes the command shorter.

## Rust Implementation Preview

The Rust workspace provides native index, workflow, lifecycle, bundle, documentation, and agent-launch commands while the installed Python `okf-agent` remains available for compatibility. From a Rust toolchain, build and run the Rust binary with:

```powershell
cargo run -- index .
cargo run -- workflow show .
cargo run -- workflow resolve . --type feature --risk strict
cargo run -- docs build .
cargo run -- docs check .
cargo run -- run codex . --no-launch
cargo run --features mcp -- mcp --repo .
cargo run -- bundle index --root .\training --index-root .\training-index
cargo run -- bundle search "lesson objectives" --root .\training --index-root .\training-index
```

The Rust index uses the same `.okf-index/knowledge.sqlite3` tables as the Python implementation. Both indexers can rebuild that disposable local index; use one writer at a time. The Rust binary is named `okf-agent-rs` and does not replace the Python-installed `okf-agent` command.

## Python and Rust Entry Points

`pipx install git+https://github.com/<owner>/okf-agent.git` continues to install the Python `okf-agent` command and importable `okf_agent` package. The Rust binary is separately installable from source:

```powershell
cargo install --git https://github.com/<owner>/okf-agent.git --bin okf-agent-rs
```

Python remains the default. Opt in to delegation for the Rust command families `index`, `update`, `validate`, `doctor`, `install`, `record-decision`, `workflow`, `bundle`, and `docs` by setting `OKF_AGENT_USE_RUST=1`. The Python launcher finds `okf-agent-rs` on `PATH`, or uses the path in `OKF_AGENT_RUST_BIN`. It checks for Rust version 0.1.1 or newer and checks the command against `okf-agent-rs capabilities`. Missing, old, or incomplete binaries produce a message and fall back to Python. `init`, `run`, `skills`, and `mcp` continue through Python until their delegation gates are met.

The release artifact matrix is `okf-agent-rs-x86_64-pc-windows-msvc.zip`, `okf-agent-rs-x86_64-unknown-linux-gnu.tar.gz`, `okf-agent-rs-aarch64-unknown-linux-gnu.tar.gz`, `okf-agent-rs-x86_64-apple-darwin.tar.gz`, and `okf-agent-rs-aarch64-apple-darwin.tar.gz`. This repository does not yet publish those binaries automatically; use `cargo install` from the source until release automation is added. Python wheels remain pure Python and do not require Rust or bundle a native executable.

Build with `--features watcher` to have `run <agent>` refresh the index while the client runs. The default Rust build has no native watcher dependency; Python `run` retains its existing polling watcher behavior.

The optional Rust MCP server is built with `--features mcp`. Repository mode defaults to `--repo .`; bundle mode requires both `--root` and `--index-root`. A default build prints an actionable feature-disabled response for `mcp`.

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

## Standalone OKF Markdown Bundles

The `bundle` profile indexes an arbitrary Markdown corpus without requiring Git. It keeps source files read-only and stores its SQLite/FTS index only in the explicitly supplied index directory. It does not create `.okf/`, repository instructions, workflow policy, or hooks.

~~~powershell
okf-agent bundle index --root .\training --index-root .\training-index
okf-agent bundle search "lesson objectives" --root .\training --index-root .\training-index
okf-agent bundle backlinks courses\intro.md --root .\training --index-root .\training-index
okf-agent bundle neighborhood courses\intro.md --depth 2 --root .\training --index-root .\training-index
okf-agent bundle validate --root .\training --index-root .\training-index
okf-agent bundle export --root .\training --index-root .\training-index
~~~

This profile reads OKF v0.2 YAML frontmatter, indexes titles, types, aliases, full metadata, Markdown body, source provenance, and directed Markdown links. Unknown types and fields are retained. Broken or ambiguous links are reported as diagnostics and do not fail standard OKF conformance. Relative links, bundle-root links beginning with `/`, escaped or percent-encoded spaces, anchors, image links, fenced code, and optional `[[wikilinks]]` are supported. The `update` command is an alias for reindexing and never writes to corpus Markdown, so hand edits remain the source of truth. `bundle export` returns nodes, directed links, provenance, and the latest validation result as JSON.

Expose the same API over MCP with `okf-agent mcp --profile bundle --root <bundle> --index-root <index>`. The repository MCP and commands remain on their existing profile. Consumers such as ConceptLoom can use the bundle CLI/MCP contract without installing repository adapters.

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

## Optional External Skills

OKF can provision selected third-party Codex plugins without vendoring their files. The built-in `default` profile installs [Matt Pocock's engineering skills](https://github.com/mattpocock/skills) and [Ponytail](https://github.com/DietrichGebert/ponytail) through their official Codex plugin marketplaces:

~~~powershell
okf-agent skills apply default
okf-agent skills doctor
~~~

This command requires the Codex CLI on `PATH` and uses `codex plugin marketplace add` and `codex plugin add`. It changes the user's Codex plugin setup, not the project. Start a new Codex task after installation. Ponytail includes lifecycle hooks; inspect and trust those hooks in Codex before enabling them. The external plugins update through Codex's marketplace/plugin lifecycle and are not pinned by OKF. To refresh marketplace sources manually, use `codex plugin marketplace upgrade`.

## Pasteable Agent Setup Prompt

Users can paste this into any terminal-capable coding agent after replacing the GitHub URL:

```text
Install OKF Repository Brain from git+https://github.com/<owner>/okf-agent.git using pipx, then run `okf-agent init .` in this repository. Do not overwrite existing agent instruction files or existing non-OKF Git hooks. After setup, run `okf-agent validate .` and report the generated adapter files.
```

Once initialized, the repository carries its own setup; collaborators only need the `okf-agent` CLI installed. `.okf/` is committed, while `.okf-index/` is local and can be deleted and rebuilt.

## Generate Human-Facing Documentation

The default local wiki is created by `init` and refreshed by `update` after it has been built once. To create a Markdown reference at another location on request, pass an output directory:

```powershell
okf-agent docs build . --output docs/reference
okf-agent docs check . --output docs/reference
```

`docs build` refreshes the SQLite index and writes a wiki with a page list, repository facts, curated `.okf/` pages, and `code-map.md`. The code map lists indexed source files, symbols, and import edges. `docs check` verifies source and generated-page hashes and exits nonzero when output is stale or changed.

The generated pages show which files and symbols exist; they do not infer product behavior or architectural meaning. Add those explanations to `.okf/` or other curated Markdown pages. The default output remains local under `.okf-index/`; choose a different output path only when you want to keep the generated reference alongside the repository documentation.

## Rust Indexing Performance

Run the repeatable synthetic benchmark with `cargo bench -p okf-cli --bench indexing`. It reports the OS and architecture, and takes five isolated-process samples for cold indexing, a second unchanged full index, 30 FTS searches, and peak working set at 250-file and 2,000-file sizes. The peak-memory measurement is available on Windows and Linux.

On Windows x86_64, the median cold-index time changed from 34 ms to 31 ms (250 files) and 189 ms to 178 ms (2,000 files) after removing an intermediate source-text allocation. The unchanged full-index medians were 27 ms to 29 ms and 190 ms to 185 ms; median peak working set was 9.37 MB to 9.34 MB and 12.27 MB to 12.37 MB. FTS medians were 1.23 ms to 1.32 ms and 3.43 ms to 3.80 ms per query. These small synthetic-run differences are noisy; they do not show a memory or search improvement. The implementation keeps the disposable full-rebuild path and existing schema, avoiding incremental-index recovery state without evidence to justify it.
