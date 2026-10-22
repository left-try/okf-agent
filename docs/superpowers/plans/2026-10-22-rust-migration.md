# OKF Agent Rust Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Rust implementation for OKF Agent's full capability set while preserving its Python package and current user-facing contracts during the transition.

**Architecture:** Follow MAE's one-way Clean Architecture boundaries: domain types, application use cases, filesystem/SQLite/Git adapters, and CLI/MCP delivery layers. Port one vertical capability at a time; keep Python authoritative for a capability until its Rust path reaches parity, then allow an explicit opt-in delegation path without making Python installation depend on a Rust toolchain.

**Tech Stack:** Cargo workspace, Rust 2024 with Rust 1.89 MSRV, `clap` 4 derive, `serde`/`serde_json`, `serde-saphyr` 2 for YAML, `rusqlite` 0.40 with bundled SQLite, optional `rmcp` 3 server + stdio transport, optional `notify` 8 watcher. Keep the workspace dependency table centralized and commit `Cargo.lock` for the CLI application. The selected YAML crate supports direct Serde deserialization and documents Rust 1.89 as its MSRV; the maintained MCP Rust SDK exposes server features and stdio transport. See the [serde-saphyr docs](https://docs.rs/crate/serde-saphyr/latest), [rusqlite docs](https://docs.rs/rusqlite/latest/rusqlite/), [clap derive docs](https://docs.rs/clap/latest/clap/_derive/index.html), [RMCP SDK](https://github.com/modelcontextprotocol/rust-sdk), and [notify docs](https://docs.rs/notify/latest/notify/).

**Spec:** [2026-10-22-rust-migration-design.md](../specs/2026-10-22-rust-migration-design.md)

## Global Constraints

- Preserve `.okf/` directory layout, frontmatter/manifest semantics, curated text, and generated-marker ownership rules.
- Keep `.okf-index/` disposable and rebuildable. Preserve the current SQLite schema and FTS behavior initially. Any schema change must be versioned, provide a safe rebuild path, and avoid corrupting an index after interruption.
- Keep CLI command names, flags, defaults, JSON field names, and exit-code meanings stable unless a change is explicitly approved.
- Keep MCP tool names, argument names/defaults, and result shapes stable. MCP remains optional and must not become a dependency of the core CLI.
- Keep bundle indexing read-only with respect to source Markdown; its explicit index directory remains the only generated bundle output.
- Preserve the Python package entry point and documented installation flow during staged migration. Distribution of Rust binaries through Python packaging must be resolved before the Python CLI delegates to Rust by default.
- Maintain platform-aware behavior for Windows, macOS, and Linux, especially path normalization, symlinks, process launching, Git hooks, and watcher behavior.
- The domain crate must not depend on filesystem, SQLite, MCP, or command-line libraries.
- Python deprecation or removal is outside this migration.

## Review Focus

- Repository paths with spaces, Unicode, Windows separators, and symlinked directories must resolve and write within the same allowed root. Own this in Tasks 2 and 6.
- An interrupted or stale SQLite index must remain rebuildable without breaking the last usable index. Own this in Tasks 4 and 5.
- Malformed YAML/frontmatter and workflow configuration must return actionable structured errors instead of panicking or silently coercing values. Own this in Tasks 2, 3, and 5.
- Python-only installations without the Rust executable must continue to support all current commands. Own this in Task 9.
- Missing optional MCP/watcher features must not prevent the default CLI from building or starting. Own this in Tasks 1, 7, and 8.

---

## File Map

- `Cargo.toml`, `Cargo.lock`: workspace members, edition/MSRV, centralized dependency versions, and CLI lockfile.
- `crates/okf-domain/src/`: validated path-independent values, workflow policy types, diagnostics, and application-facing contracts.
- `crates/okf-application/src/`: use cases and ports shared by CLI and MCP.
- `crates/okf-adapter-fs/src/`: safe root resolution, YAML parsing support, templates, generated blocks, documentation rendering, and file writes.
- `crates/okf-adapter-sqlite/src/`: repository and bundle SQLite schemas, FTS queries, transaction boundaries, and rebuilds.
- `crates/okf-adapter-git/src/`: Git root and changed-file detection.
- `crates/okf-adapter-mcp/src/`: optional MCP server transport and tool registration.
- `crates/okf-cli/src/`: command parsing, JSON output, exit codes, and process/client launching.
- `pyproject.toml`, `okf_agent/`: preserve the existing Python package and entry point; add delegation only after explicit parity and packaging gates.
- `README.md`: document the Rust binary's provisional install/run path and Python compatibility behavior.
- `crates/*/tests/`: Rust contract and adapter tests, added in the task that owns each behavior.

## Interfaces

- `okf_domain::diagnostics::{Diagnostic, ErrorKind}`: stable structured errors with a code, message, and optional repository-relative path.
- `okf_domain::paths::RepoPath::new(root: &Path, input: &Path) -> Result<RepoPath, Diagnostic>`: accepts only a path resolving within `root` and stores a normalized repository-relative path.
- `okf_application::ports::{Repository, SourceIndex, BundleIndex, KnowledgeStore}`: object-safe boundaries for filesystem and index implementations.
- `okf_application::workflow::resolve(policy: WorkflowPolicy, assessment: WorkflowAssessment, capabilities: WorkflowCapabilities) -> Result<WorkflowDecision, Diagnostic>`.
- `okf_application::repository_index::{build(root: &Path) -> Result<IndexReport, Diagnostic>, search(root: &Path, query: &str, limit: usize) -> Result<Vec<SearchHit>, Diagnostic>}`.
- `okf_application::bundle::{index(root: &Path, index_root: &Path, wikilinks: bool) -> Result<BundleIndexReport, Diagnostic>, search(index_root: &Path, query: &str, limit: usize) -> Result<Vec<BundleSearchHit>, Diagnostic>, graph(index_root: &Path, path: &str, depth: usize, direction: GraphDirection) -> Result<GraphResult, Diagnostic>, export(index_root: &Path) -> Result<BundleExport, Diagnostic>}`.
- `okf_application::lifecycle::{initialize(root: &Path, profiles: &[String]) -> Result<InitReport, Diagnostic>, update(root: &Path, changed: Option<&[RepoPath]>) -> Result<UpdateReport, Diagnostic>, validate(root: &Path) -> Result<ValidationReport, Diagnostic>}`.
- CLI binary during transition: `okf-agent-rs`; do not replace the Python-installed `okf-agent` command before Task 9's explicit gate.

---

## Task 1: Establish the Rust Workspace and CLI Shell

**Files:**
- Create: `Cargo.toml`, `Cargo.lock`
- Create: `crates/okf-domain/Cargo.toml`, `crates/okf-domain/src/lib.rs`
- Create: `crates/okf-application/Cargo.toml`, `crates/okf-application/src/lib.rs`
- Create: `crates/okf-adapter-fs/Cargo.toml`, `crates/okf-adapter-fs/src/lib.rs`
- Create: `crates/okf-adapter-sqlite/Cargo.toml`, `crates/okf-adapter-sqlite/src/lib.rs`
- Create: `crates/okf-adapter-git/Cargo.toml`, `crates/okf-adapter-git/src/lib.rs`
- Create: `crates/okf-adapter-mcp/Cargo.toml`, `crates/okf-adapter-mcp/src/lib.rs`
- Create: `crates/okf-cli/Cargo.toml`, `crates/okf-cli/src/main.rs`, `crates/okf-cli/src/commands.rs`
- Test: `crates/okf-cli/tests/command_contract.rs`

**Interfaces:**
- Produces the workspace and dependency direction described above.
- `okf-cli` initially exposes `--version`, help, and an explicit `capabilities` command reporting which provisional Rust commands exist; it returns a clear unsupported-command error for unported capabilities.
- Build RMCP and notify only behind Cargo features; default CLI must not compile them.

- [ ] **Step 1: Add workspace manifests and the seven named crates** with workspace package metadata, Rust edition 2024, `rust-version = "1.89"`, and shared dependency declarations. Add `okf-cli` as the sole default binary, named `okf-agent-rs`.
- [ ] **Step 2: Add minimal crate roots and dependency-direction checks** so domain has no adapter dependencies, application depends only on domain, adapters depend on domain/application, and CLI depends on application plus adapters.
- [ ] **Step 3: Define the CLI shell** with clap derive, JSON diagnostic output, and stable nonzero status for unsupported commands.
- [ ] **Step 4: Add the command contract test** asserting binary name, help availability, `--version`, and unsupported-command JSON/error behavior.
- [ ] **Step 5: Run `cargo fmt --all -- --check`, `cargo check --workspace`, and `cargo test --workspace`**; require all to pass before committing `build: add Rust workspace and CLI shell`.

## Task 2: Implement Domain Diagnostics, Repository Paths, and YAML Parsing

**Files:**
- Create: `crates/okf-domain/src/diagnostics.rs`, `paths.rs`, `workflow.rs`
- Create: `crates/okf-adapter-fs/src/paths.rs`, `yaml.rs`
- Modify: crate roots and manifests
- Test: `crates/okf-domain/tests/repository_paths.rs`, `crates/okf-domain/tests/workflow_policy.rs`, `crates/okf-adapter-fs/tests/yaml_values.rs`

**Interfaces:**
- Produces `Diagnostic`, `ErrorKind`, `RepoPath`, workflow enums/structs, and `parse_yaml<T: DeserializeOwned>(source: &str, path: &RepoPath) -> Result<T, Diagnostic>`.
- `RepoPath` comparisons and display use normalized forward-slash repository-relative strings on all platforms.

- [ ] **Step 1: Add `RepoPath` contract tests** for relative input, absolute in-root input, spaces/Unicode, `..` traversal, Windows drive paths, and symlinks resolving outside the root.
- [ ] **Step 2: Implement `RepoPath::new` and diagnostic serialization** using canonicalized existing ancestors for not-yet-created outputs; reject escapes before any write.
- [ ] **Step 3: Add workflow policy JSON deserialization checks** for unknown keys, missing required values, and invalid enum values; the current `.okf/workflows/config.json` contract is JSON.
- [ ] **Step 4: Add generic YAML value parsing checks** for a valid typed value and malformed YAML, then implement `parse_yaml` with `serde-saphyr` and translate parser locations into path-aware diagnostics. Keep YAML parsing at adapter boundaries.
- [ ] **Step 5: Run the focused Rust tests and `cargo clippy --workspace --all-targets -- -D warnings`**; require clear path/config failures and no warnings before committing `feat: add Rust domain and path contracts`.

## Task 3: Port Workflow Resolution as the First Complete Vertical Slice

**Files:**
- Create: `crates/okf-application/src/workflow.rs`
- Create: `crates/okf-cli/src/commands/workflow.rs`
- Modify: `crates/okf-cli/src/commands.rs`, `crates/okf-application/src/lib.rs`
- Test: `crates/okf-application/tests/workflow_resolution.rs`, `crates/okf-cli/tests/workflow_cli.rs`

**Interfaces:**
- Consumes Task 2 `WorkflowPolicy`, `WorkflowAssessment`, and `WorkflowCapabilities`.
- Produces the exact `resolve(...) -> Result<WorkflowDecision, Diagnostic>` interface above.
- Supports `workflow show`, `workflow explain`, and `workflow resolve` with the Python CLI's choices, defaults, and JSON result fields.

- [ ] **Step 1: Add resolution cases** for feature/bugfix/docs classifications, explicit and inferred risk, high-impact strict forcing, method selection, ambiguous/behavioral/stateful signals, and single-agent versus independent-role capability.
- [ ] **Step 2: Implement the pure policy resolver** without filesystem or CLI dependencies.
- [ ] **Step 3: Add CLI argument and output checks** comparing command names, accepted values, defaults, and exit-code classes with `okf-agent workflow`.
- [ ] **Step 4: Wire policy loading through the filesystem adapter and expose the three Rust workflow commands** while leaving Python workflow behavior unchanged.
- [ ] **Step 5: Run focused tests, `cargo check --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings`**; commit as `feat: port workflow resolution to Rust`.

## Task 4: Port Repository Source Discovery, SQLite Indexing, and Search

**Files:**
- Create: `crates/okf-adapter-fs/src/discovery.rs`
- Create: `crates/okf-adapter-sqlite/src/repository.rs`
- Create: `crates/okf-application/src/repository_index.rs`
- Create: `crates/okf-cli/src/commands/index.rs`
- Modify: crate roots/manifests and `README.md`
- Test: `crates/okf-adapter-sqlite/tests/repository_index.rs`, `crates/okf-application/tests/repository_search.rs`

**Interfaces:**
- Produces the `build` and `search` interfaces above plus deterministic `detect(root)` and `indexed_content(root)` reads.
- Preserve current skip directories, source suffixes, manifest names, symbol/import extraction behavior, FTS5 search behavior, and table/column names for `.okf-index/knowledge.sqlite3`.
- Include `.rs` and `Cargo.toml` in source discovery without dropping any existing language or manifest.

- [ ] **Step 1: Add discovery/index contract cases** for deterministic relative paths, duplicate symbols, import edges, all existing skip directories, Rust files/manifests, and empty repositories.
- [ ] **Step 2: Implement bounded file enumeration and source extraction** behind `okf-adapter-fs`; keep the initial regex extraction behavior compatible and isolate it for later parser upgrades.
- [ ] **Step 3: Implement transactional SQLite schema setup/rebuild** in `okf-adapter-sqlite`; build a replacement index inside a transaction so failures do not leave half-populated tables.
- [ ] **Step 4: Implement search, symbol lookup, and indexed-content reads** with the existing FTS query/fallback semantics and stable ordering.
- [ ] **Step 5: Expose Rust `index` command output and update generated language facts** without editing curated `.okf/` prose.
- [ ] **Step 6: Run focused tests, `cargo check --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings`**; commit as `feat: port repository indexing to Rust`.

## Task 5: Port Standalone Markdown Bundle Operations

**Files:**
- Create: `crates/okf-adapter-fs/src/markdown_links.rs`, `frontmatter.rs`
- Create: `crates/okf-adapter-sqlite/src/bundle.rs`
- Create: `crates/okf-application/src/bundle.rs`
- Create: `crates/okf-cli/src/commands/bundle.rs`
- Modify: crate roots/manifests and `README.md`
- Test: `crates/okf-adapter-fs/tests/bundle_markdown.rs`, `crates/okf-adapter-sqlite/tests/bundle_index.rs`, `crates/okf-cli/tests/bundle_cli.rs`

**Interfaces:**
- Produces `index`, `search`, `graph`, and `export` interfaces above.
- Preserve current frontmatter metadata retention, source provenance, edge kinds, diagnostics, `[[wikilinks]]` opt-in, and bundle JSON field names.
- Bundle operations must never write to corpus Markdown.

- [ ] **Step 1: Add link/frontmatter cases** for relative/root links, URL-encoded spaces, fragments, images, fenced code, wikilinks opt-in, unknown metadata/types, ambiguous/broken links, and malformed YAML.
- [ ] **Step 2: Implement Markdown target extraction and path resolution** with deterministic sorting and no source writes.
- [ ] **Step 3: Implement the bundle SQLite adapter and graph queries** in the explicitly supplied index root.
- [ ] **Step 4: Implement bundle validation, FTS search, backlinks, neighborhood, and JSON export** with the current result shapes.
- [ ] **Step 5: Expose `bundle index|update|validate|search|backlinks|neighborhood|export`** and stable exit codes.
- [ ] **Step 6: Run focused tests, `cargo check --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings`**; commit as `feat: port bundle profile to Rust`.

## Task 6: Port Repository Lifecycle, Generated Knowledge, and Safe Adapter Installation

**Files:**
- Create: `crates/okf-application/src/lifecycle.rs`, `knowledge.rs`, `enforcement.rs`
- Create: `crates/okf-adapter-fs/src/templates.rs`, `generated_blocks.rs`, `instructions.rs`
- Create: `crates/okf-adapter-git/src/lib.rs`
- Create: `crates/okf-cli/src/commands/lifecycle.rs`
- Modify: crate roots/manifests
- Test: `crates/okf-adapter-fs/tests/generated_files.rs`, `crates/okf-application/tests/lifecycle.rs`, `crates/okf-cli/tests/lifecycle_cli.rs`

**Interfaces:**
- Produces `initialize`, `update`, and `validate` interfaces above plus `doctor`, `record_decision`, and `install` use cases.
- Preserve existing adapters for AGENTS.md, CLAUDE.md, Cursor, Gemini, Git hooks, workflow profiles, and Codex skill installation.
- Only OKF-owned generated blocks may be replaced; non-OKF hooks and curated text must remain intact.

- [ ] **Step 1: Add filesystem/lifecycle cases** for already-initialized repositories, partial `.okf/`, malformed manifest, curated text, duplicate/malformed markers, symlinked outputs, and non-OKF hooks.
- [ ] **Step 2: Implement templates, generated-block replacement, and repository-bounded writes** in the filesystem adapter.
- [ ] **Step 3: Implement Git root/changed-file discovery and safe index/knowledge refresh** through injected repository/index ports.
- [ ] **Step 4: Implement init, update, validate, doctor, record-decision, and Codex skill install use cases** with structured reports matching current JSON names and `--skills-dir` behavior.
- [ ] **Step 5: Expose the lifecycle commands and preserve Python as the current default entry point**; compare representative outputs against existing command behavior.
- [ ] **Step 6: Run focused tests, `cargo check --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings`**; commit as `feat: port repository lifecycle to Rust`.

## Task 7: Port Documentation Generation, Local Web UI, and Watcher/Run Behavior

**Files:**
- Create: `crates/okf-application/src/docs.rs`, `watcher.rs`, `run_agent.rs`
- Create: `crates/okf-adapter-fs/src/docs.rs`, `web.rs`
- Create: `crates/okf-cli/src/commands/docs.rs`, `run.rs`
- Modify: crate manifests, adding `notify` only behind an optional watcher feature
- Test: `crates/okf-application/tests/docs_generation.rs`, `crates/okf-adapter-fs/tests/docs_web.rs`, `crates/okf-cli/tests/run_cli.rs`

**Interfaces:**
- Produces `docs build`, `docs check`, `docs serve`, and `run <agent>` with current defaults and output fields.
- Keep docs generation deterministic, limited to managed outputs/blocks, and opt-in where the current contract says it is opt-in.
- The run command must preserve current path, client alias, `--no-launch`, and child exit-code behavior.

- [ ] **Step 1: Add docs behavior cases** for empty indexes, unchanged repeat builds, preserved curated text, stale check read-only behavior, unsafe output paths, and safe Markdown route resolution.
- [ ] **Step 2: Implement documentation rendering/build/check** over indexed-content ports, with no database rewrite from check mode.
- [ ] **Step 3: Implement the local read-only server** bound to loopback by default and protected against traversal/symlink escapes.
- [ ] **Step 4: Implement the opt-in watcher** with debouncing/coalescing and periodic reconciliation because native file events can be missed.
- [ ] **Step 5: Implement client launch aliases and `--no-launch`** with child exit propagation and watcher shutdown on every exit path.
- [ ] **Step 6: Run focused tests, `cargo check --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings`**; commit as `feat: port docs and run support to Rust`.

## Task 8: Add Optional Rust MCP Adapters

**Files:**
- Create: `crates/okf-adapter-mcp/src/repository.rs`, `bundle.rs`
- Modify: `crates/okf-adapter-mcp/Cargo.toml`, `crates/okf-cli/Cargo.toml`, `crates/okf-cli/src/commands/mcp.rs`
- Test: `crates/okf-adapter-mcp/tests/tool_contract.rs`

**Interfaces:**
- Produces repository and bundle MCP profiles over the same application services as CLI.
- Preserve tool names: `okf.initialize`, `okf.status`, `okf.search`, `okf.get_concept`, `okf.get_change_context`, `okf.get_workflow_policy`, `okf.resolve_workflow`, `okf.record_decision`, `okf.update_after_change`, `okf.validate`, `okf.reindex`, and the existing `okf.bundle_*` names.
- Require explicit roots for bundle mode; default repository root remains `.`.

- [ ] **Step 1: Add tool contract cases** for names, argument defaults, result field names, invalid concept path rejection, and optional-feature absence.
- [ ] **Step 2: Register repository and bundle tools using RMCP** with stdio transport and application service dependencies.
- [ ] **Step 3: Gate MCP command compilation behind the `mcp` feature** and provide an actionable error when invoked from a build without it.
- [ ] **Step 4: Run focused tests and default/feature-enabled `cargo check` plus Clippy**; commit as `feat: add optional Rust MCP server`.

## Task 9: Preserve Python Compatibility and Define the Rust Distribution Gate

**Files:**
- Modify: `pyproject.toml`, `README.md`, Python facade only where delegation is approved by the parity gate
- Create: `crates/okf-cli/tests/python_compatibility.rs` or an integration script under `scripts/`
- Modify: package/build documentation

**Interfaces:**
- `pipx install git+https://.../okf-agent.git` continues to install the Python CLI and existing Python imports.
- `okf-agent-rs` is separately installable during transition; no Python workflow assumes Rust is installed.
- Delegation is opt-in until all command families have parity and release artifacts are available for Windows, macOS, and Linux.

- [ ] **Step 1: Document the two entry points and supported capability matrix** so users can distinguish Python-only, Rust-only, and mixed installations.
- [ ] **Step 2: Define release artifacts and discoverable binary selection** without changing the existing `okf-agent` console script default.
- [ ] **Step 3: Add opt-in delegation for only completed commands**; if the binary is absent or too old, retain the Python path and return a clear diagnostic.
- [ ] **Step 4: Confirm Python API imports and Python-only installation remain unchanged**; make default alias switching a later release decision requiring explicit approval.
- [ ] **Step 5: Run packaging/build checks on the supported operating systems when available**; commit as `feat: add staged Rust CLI distribution`.

## Task 10: Measure and Apply Evidence-Based Indexing Improvements

**Files:**
- Modify: `crates/okf-adapter-fs/src/discovery.rs`, `crates/okf-adapter-sqlite/src/repository.rs`
- Create: `crates/okf-cli/benches/indexing.rs` and benchmark fixture generator
- Modify: `README.md` with measured limits and rebuild guidance

**Interfaces:**
- Full rebuild remains the fallback and correctness source.
- Incremental updates must account for additions, deletions, renames, and content changes and recover to a full rebuild on stale/corrupt metadata.

- [ ] **Step 1: Add repeatable benchmark cases** for cold full index, unchanged repeated index, FTS search, and peak memory at small and larger repository sizes.
- [ ] **Step 2: Record baseline runtime and memory by platform** before selecting an optimization.
- [ ] **Step 3: Optimize the measured hot path** using bounded batches or parallel reads while preserving deterministic results.
- [ ] **Step 4: Add incremental metadata only if the measured gain justifies its schema and recovery cost**; otherwise keep the measured batching optimization and full rebuild.
- [ ] **Step 5: Compare before/after benchmark results and document the tradeoff**; commit as `perf: optimize measured indexing bottleneck`.

## Completion Criteria

- Every command family and both MCP profiles have Rust implementations over shared application use cases.
- The existing Python package and `okf-agent` entry point continue to work during the transition.
- Rust and Python behavior agree on CLI/JSON contracts for each capability before that capability is delegated.
- Repository and bundle databases remain rebuildable; bundle Markdown remains read-only.
- Rust builds with default optional features disabled and with MCP/watcher features enabled.
- Performance changes have repeatable before/after measurements; no incremental path removes the full rebuild fallback.
- `.okf/` curated sections and unrelated agent/user files remain preserved.
