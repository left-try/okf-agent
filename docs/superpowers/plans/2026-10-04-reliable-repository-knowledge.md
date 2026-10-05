# Reliable Repository Knowledge and Local Wiki Implementation Plan

> **For agentic workers:** Use `superpowers:executing-plans` and execute task by task. Each checkbox is a reviewable deliverable.

**Goal:** Make OKF knowledge updates dependable and provide a local, searchable wiki view whose pages can be checked for freshness against repository changes.

**Architecture:** Keep Markdown in `.okf/` as the human-reviewable source of truth. Fix update, indexing, validation, and hooks first; then add deterministic documentation generation and a lightweight local web UI. AI-generated explanations and external publication are outside this plan.

**Tech Stack:** Python 3.11+, current standard-library CLI and SQLite FTS index; add no required runtime dependencies for the first iteration.

**Spec:** Product decisions and findings in this document.

## Scope and Product Decisions

- GitHub Wiki, GitHub Pages, and other hosted publication are postponed.
- Repository Markdown is authoritative; generated pages must link to the source files they describe.
- Deterministic extraction may generate facts and navigation. Explanatory or architectural claims remain curated Markdown unless a human reviews them.
- The local wiki must run without a hosted service and must not expose repository files outside its documentation surface.
- `okf-agent docs check` must return a nonzero exit status when generated pages are stale or invalid.
- `init` builds the local wiki; `update` rebuilds it when present; `validate` blocks stale generated wiki output before push.

## Current Errors and Recommendations

| Priority | Finding in current code | Recommendation |
|---|---|---|
| P1 | `lifecycle.refresh_knowledge()` updates `.okf/` but does not rebuild `.okf-index/`; searches can remain stale after `update`. | Make update refresh both knowledge facts and the local index, with a single clear lifecycle entry point. |
| P1 | `lifecycle.git_changed()` uses `git diff --name-only HEAD`, which omits untracked files. | Collect staged, unstaged, and untracked paths, normalize and deduplicate them, and tolerate non-Git directories. |
| P1 | The generated `pre-commit` hook runs `update` with `|| exit 0`, hiding failures. | Preserve existing hooks, report OKF failures, and define explicit fail-open/fail-closed behavior; default validation hooks should not silently claim success. |
| P1 | `cli.run` computes context but does not pass it to the launched client; the watcher only runs in this command. | State clearly that the payload is not injected; clients receive the native repository instructions. Ensure background workers stop cleanly. |
| P2 | `validation.validate()` checks required files and marker closure only. | Validate manifest structure, generated block pairing, documentation links, and index/document freshness. Return actionable errors. |
| P2 | `discovery.detect()` recognizes some manifests that `discovery.files()` does not index, including `go.mod`, `Cargo.toml`, and `pom.xml`. | Align discovery and indexing coverage; add focused fixture tests for each supported language and manifest. |
| P2 | `watcher.start()` does not handle files disappearing between enumeration and `stat()`. | Make polling resilient to filesystem races, expose stop/join behavior, and report indexing errors instead of silently swallowing them. |
| P2 | `refresh_knowledge()` rewrites a timestamp on each run, causing noise even when source facts did not change. | Write generated content only when its substantive content changes; keep build timestamps in metadata if needed. |
| P3 | No local wiki renderer or docs freshness command exists. | Add a local Markdown documentation site and a check mode suitable for later CI integration. |

## Global Constraints

- Python requirement remains `>=3.11`.
- Core CLI must keep working with no optional dependencies installed.
- Preserve user-authored content outside OKF-generated blocks.
- Do not overwrite existing non-OKF Git hooks.
- Do not publish to GitHub or require network access.
- Never serve arbitrary repository paths through the local documentation server.

## Review Focus

- Git repositories with staged, unstaged, and untracked files must all report the correct changed paths.
- Non-Git folders and missing `HEAD` must not crash update or index operations.
- Files deleted during watcher scans must not terminate the watcher thread.
- Malformed Markdown links and unclosed generated blocks must produce useful validation errors.
- Web requests for traversal paths, symlink escapes, and non-documentation files must not read arbitrary local files.

## Files and Responsibilities

- `okf_agent/lifecycle.py`: changed-file collection and update orchestration.
- `okf_agent/watcher.py`: resilient watcher lifecycle and index refresh.
- `okf_agent/enforcement.py`: safe Git hook installation and hook behavior.
- `okf_agent/validation.py`: knowledge and documentation validation.
- `okf_agent/discovery.py`: consistent source and manifest discovery.
- `okf_agent/cli.py`: `docs build`, `docs check`, and `docs serve` commands.
- `okf_agent/docs.py` (new): deterministic Markdown page generation and freshness metadata.
- `okf_agent/web.py` (new): local read-only wiki server and safe routes.
- `tests/test_lifecycle.py`, `tests/test_onboarding.py`, `tests/test_docs.py` (new), `tests/test_web.py` (new): regression and feature coverage.
- `docs/superpowers/plans/2026-10-04-reliable-repository-knowledge.md`: this execution plan.

## Implementation Plan

### Phase 1: Make update and indexing reliable

#### Task 1: Include all changed files in lifecycle updates

**Files:** `okf_agent/lifecycle.py`, `tests/test_lifecycle.py`

- [x] Add tests for staged, unstaged, and untracked paths, plus a directory without Git metadata.
- [x] Implement a changed-path collector that merges Git status sources, normalizes paths relative to the repository root, and deduplicates results.
- [x] Make `refresh_knowledge()` call `build_index(root)` after refreshing generated facts.
- [x] Run `python -m unittest discover -s tests -p test_lifecycle.py -v`; all lifecycle tests pass.

#### Task 2: Make watcher shutdown and polling reliable

**Files:** `okf_agent/watcher.py`, `okf_agent/cli.py`, `tests/test_lifecycle.py`

- [x] Add tests for a disappearing file, an indexing error, and stopping the watcher.
- [x] Return a watcher handle that can stop and join the worker; handle `OSError` per-file without terminating the loop.
- [x] Stop and join the watcher in `cli.run` after the client process exits, including when launch fails.
- [x] Run the lifecycle tests and confirm the worker exits within its bounded join timeout.

#### Task 3: Align language and manifest discovery

**Files:** `okf_agent/discovery.py`, `tests/test_lifecycle.py`

- [x] Add fixtures for Python, JavaScript/TypeScript, Go, Rust, and Java manifests and representative source extensions.
- [x] Include every manifest recognized by `detect()` in the indexer's document set.
- [x] Make reported language/framework hints match files actually indexed; unsupported frameworks must not be guessed from a manifest alone.
- [x] Run discovery and lifecycle tests; assert each fixture reports and indexes its expected manifest.

### Phase 2: Make validation and automation trustworthy

#### Task 4: Expand validation with actionable results

**Files:** `okf_agent/validation.py`, `tests/test_lifecycle.py`

- [x] Add tests for invalid or duplicate manifest fields, unmatched start/end markers, missing/stale index, and broken local Markdown links.
- [x] Validate `schema_version`, generated marker pairing/order, known relative links, and index presence/freshness.
- [x] Keep `validate()` result keys `ok`, `errors`, and `warnings` stable; include repository-relative paths in diagnostics.
- [x] Run lifecycle tests; invalid fixtures fail with the expected message and valid initialized fixtures pass.

#### Task 5: Repair Git hook failure behavior and make generated writes idempotent

**Files:** `okf_agent/enforcement.py`, `okf_agent/knowledge.py`, `okf_agent/lifecycle.py`, `tests/test_lifecycle.py`

- [x] Add tests proving that existing custom hooks remain unchanged, OKF hooks surface command failures, and repeated updates without source changes do not rewrite generated files.
- [x] Remove unconditional success masking from the generated `pre-commit` hook and document the hook's exit behavior.
- [x] Only replace generated blocks when generated content differs; do not update timestamps on content-identical runs.
- [x] Record cross-platform CI as follow-up work; no CI workflow exists in this repository yet.

#### Task 6: Make `run` behavior honest and deterministic

**Ruling:** Keep launching the requested executable for backwards compatibility, but print the context-delivery limitation before launch. Per-client prompt injection is deferred until reliable client-specific adapters are designed.

**Files:** `okf_agent/cli.py`, `okf_agent/onboarding.py`, `tests/test_onboarding.py`

- [x] Add tests for `--no-launch`, missing executable, and child-process exit status.
- [x] Report clearly in CLI output that computed context is not injected; native repository instructions remain the client integration.
- [x] Keep the context-delivery limitation explicit in CLI output and README examples.
- [x] Run onboarding tests and verify the CLI's exit status matches the child process or client error.

### Phase 3: Generate and browse local documentation

#### Task 7: Generate deterministic documentation pages

**Files:** `okf_agent/docs.py` (new), `okf_agent/cli.py`, `tests/test_docs.py` (new)

- [x] Add tests for generated page paths, navigation, source links, and repeatable output.
- [x] Define `build_docs(root: Path, output: Path | None = None) -> dict[str, object]` to build pages from `.okf/` Markdown and detected repository facts.
- [x] Store a manifest containing source paths and content hashes so freshness checks do not rely only on timestamps; include indexed source files as freshness inputs.
- [x] Add `okf-agent docs build` and `okf-agent docs check`; `check` returns nonzero when inputs changed since the last build.
- [x] Run docs tests; two builds from unchanged input produce byte-identical files.

#### Task 8: Add a safe local wiki UI

**Files:** `okf_agent/web.py` (new), `okf_agent/cli.py`, `tests/test_web.py` (new)

- [x] Add tests for page rendering, navigation/search, unknown routes, traversal attempts, and paths resolving outside the site root.
- [x] Implement `serve_docs(root: Path, host: str = "127.0.0.1", port: int = 0) -> None` using the Python standard library and only serve generated documentation pages.
- [x] Add `okf-agent docs serve` with loopback binding by default and a printed local URL.
- [x] Run web tests; valid pages render and out-of-scope paths return 404. A real symlink test is skipped on this Windows host because symlink creation is unavailable; the resolver escape test covers the same path check.

#### Task 9: Document the supported workflow

**Files:** `README.md`, `tests/test_onboarding.py`

- [x] Document init, update, validate, docs build/check/serve, supported client behavior, and current language coverage.
- [x] Explain that generated facts are deterministic and curated explanations require review.
- [x] Add a CLI help smoke test for nested `docs` commands.
- [x] Run the full unittest suite and follow the README steps in a temporary repository.

## Deferred Work

- GitHub Wiki export and GitHub Pages publication.
- Cross-platform CI workflow and CI enforcement.
- Hosted wiki service, authentication, or multi-user editing.
- AI-authored architectural explanations and autonomous documentation rewrites.
- Broad language/framework coverage beyond the initial supported manifest and source list.
- CI setup may be added after local commands and validation behavior are stable.
