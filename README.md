# OKF Repository Brain

`okf-agent` gives coding agents a repository-owned knowledge bundle in `.okf/` and a rebuildable local index in `.okf-index/`. It makes the same lifecycle available to Codex, Claude Code, Cursor, Gemini CLI, and any agent that follows repository instructions: retrieve context before a change, update knowledge after it, and validate before completion.

## Install

The supported `okf-agent` CLI is the Rust binary. Install it from the repository with a Rust toolchain:

```powershell
cargo install --git https://github.com/left-try/okf-agent.git --locked --bin okf-agent
```

Check the GitHub Releases page for prebuilt binaries. When a matching binary is not available, install directly from the repository with Cargo as shown above.

The Python package remains available for existing `okf_agent` imports, but it no longer installs an `okf-agent` command. The Rust CLI is the only supported command-line implementation.

## Commands

From a checkout, build and run the Rust CLI with:

```powershell
cargo run --locked -- index .
cargo run --locked -- workflow show .
cargo run --locked -- workflow resolve . --type feature --risk strict
cargo run --locked -- docs build .
cargo run --locked -- docs check .
cargo run --locked -- run codex . --no-launch
cargo run --locked -- skills doctor
cargo run --locked -- bundle index --root .\training --index-root .\training-index
cargo run --locked -- bundle search "lesson objectives" --root .\training --index-root .\training-index
```

The Rust index uses `.okf-index/knowledge.sqlite3` as its disposable local index. `init` creates it, builds the local wiki, and adds `.okf-index/` to `.gitignore` while preserving existing content.

Build with `--features watcher` to have `run <agent>` refresh the index while the client runs. Install the optional MCP server with `cargo install --git https://github.com/left-try/okf-agent.git --locked --features mcp --bin okf-agent`. Repository mode defaults to `--repo .`; bundle mode requires both `--root` and `--index-root`.

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

Initialization adds `.okf-index/` to the repository's `.gitignore` so the rebuildable local index stays out of version control. Existing `.gitignore` content is preserved.

The Rust `okf-agent` CLI includes the `docs` and `bundle` command groups and the `init --profiles` option documented below. Reinstall with `cargo install --git https://github.com/left-try/okf-agent.git --locked --bin okf-agent` to receive the current commands.

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
Install OKF Repository Brain with Cargo, then run `okf-agent init .` in this repository. Do not overwrite existing agent instruction files or existing non-OKF Git hooks. After setup, run `okf-agent validate .` and report the generated adapter files.
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

Run the repeatable synthetic benchmark with `cargo bench -p okf-cli --bench indexing`. It reports the OS and architecture, and takes five isolated-process samples for cold indexing, a second unchanged full index, 30 FTS searches, and peak working set at 250-file and 2,000-file sizes. The peak-memory measurement is available on Windows and Linux. Recorded baseline and after samples are in [windows-x86_64.json](crates/okf-cli/benches/results/windows-x86_64.json).

On Windows x86_64, the median cold-index time changed from 34 ms to 34 ms (250 files) and 189 ms to 175 ms (2,000 files) after removing an intermediate source-text allocation. The unchanged full-index medians were 27 ms to 30 ms and 190 ms to 182 ms; median peak working set was 9.37 MB to 9.41 MB and 12.27 MB to 12.46 MB. FTS medians were 1.23 ms to 1.47 ms and 3.43 ms to 4.18 ms per query. The larger cold-index case improved by about 7%, while the smaller case did not; memory and search show no improvement, and timings are noisy. The implementation keeps the disposable full-rebuild path and existing schema, avoiding incremental-index recovery state without evidence to justify it.
