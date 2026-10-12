# Index-Backed Human Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an explicit, repeatable workflow for generating and checking a small set of useful human-facing `/docs` reference pages from OKF's local code index.

**Architecture:** Keep `.okf/` as the curated knowledge source and `.okf-index/knowledge.sqlite3` as the disposable, rebuildable source map. `okf-agent docs build` refreshes the index and writes deterministic reference pages under `docs/reference/`; `okf-agent docs check` compares the expected generated content with the checked-in pages and reports stale or missing pages without changing them. Generated content is bounded to verifiable repository facts and links; explanations remain human-authored. Neither Git hooks nor normal agent startup generate or rewrite `/docs`.

**Tech Stack:** Python 3.11+, standard library, SQLite, Markdown, unittest.

**Spec:** This plan refines the documentation-generation decisions in `docs/superpowers/plans/2026-10-04-reliable-repository-knowledge.md` for index-derived reference pages and explicit invocation.

## Global Constraints

- Python requirement remains `>=3.11`.
- Core CLI must keep working with no optional dependencies installed.
- `.okf/` remains the curated, repository-owned knowledge source; `.okf-index/` remains disposable and rebuildable.
- Preserve user-authored Markdown outside OKF-generated blocks.
- Generated documentation must be deterministic and link back to repository source files.
- Generation occurs only through an explicit `okf-agent docs build` invocation; no Git hook, watcher, `init`, `run`, or `update` may write `/docs`.
- Do not generate unsupported product, behavior, security, or architecture claims from symbol names or import edges.
- Do not publish docs or add a web server in this feature.

## Test-First Plan

Implementation follows one strict RED-GREEN-REFACTOR cycle per behavior. Write the test and run it before writing production code; confirm it fails because the required behavior is missing. Then add the smallest implementation that makes it pass, rerun the focused test, and run the relevant existing tests before moving on. A test that errors from bad setup or passes before implementation does not count as RED; fix the test and observe the expected failure first.

Write the tests in the following order, before their corresponding implementation steps:

| Order | Test(s) | Required RED behavior | Later implementation |
|---|---|---|---|
| 1 | `test_indexed_content_returns_sorted_documents_symbols_and_edges`; `test_indexed_content_missing_index_is_read_only` | Import/API missing or expected index data unavailable | `discovery.indexed_content()` |
| 2 | `test_rendered_pages_link_curated_overview_and_indexed_code`; `test_rendered_pages_do_not_infer_behavior`; `test_rendered_pages_handle_empty_index` | Renderer/API missing; output assertions define pages, links, and bounded claims | Pure render functions in `docs.py` |
| 3 | `test_build_docs_creates_only_reference_pages`; `test_build_docs_preserves_curated_text_outside_markers`; `test_build_docs_leaves_unrelated_docs_untouched`; `test_build_docs_is_byte_deterministic`; `test_build_docs_rejects_page_symlink_outside_repository` | Build API missing, expected generated output/preservation absent, or symlink target not protected | `build_docs()` |
| 4 | `test_check_docs_detects_missing_and_stale_pages`; `test_check_docs_does_not_write_docs_or_persistent_index` | Check API missing or stale state is not reported/read-only guarantee fails | `check_docs()` |
| 5 | `test_docs_cli_build_and_check_exit_codes`; `test_docs_cli_resolves_output_within_repository`; `test_docs_help_lists_subcommands` | CLI commands missing or wrong exit/output behavior | Nested CLI commands |
| 6 | `test_lifecycle_commands_do_not_generate_docs`; `test_validate_rejects_malformed_generated_markers_and_broken_links` | Existing flows create docs or validator accepts invalid generated pages | Opt-in boundary and validation |

Each row is implemented as a small cycle rather than writing all production code first. Keep the expected failure visible in the work log/commit description; do not weaken the assertion just to get a green test. New tests should exercise real files and a temporary repository. Avoid mocks except where a failure condition cannot be produced through the public interface.

## Review Focus

- Missing or malformed SQLite indexes: `docs build` rebuilds the index; `docs check` gives a nonzero result and tells the user how to refresh docs.
- Code paths containing spaces or URL-sensitive characters: generated links remain valid relative Markdown links.
- Existing `docs/` content, including plans and hand-written pages: generation touches only its documented output files and generated blocks.
- Output directories and managed page paths that contain symlinks: build/check refuse links so they cannot read or overwrite files through redirected paths.
- Empty repositories and repositories with no supported symbols: pages remain valid and say the index found no entries.
- Repeated builds with unchanged indexed inputs: output is byte-identical and does not introduce timestamps or random identifiers.

---

## File Map

- `okf_agent/discovery.py`: add a stable reader for indexed documents and symbols; keep schema/query details out of the renderer.
- `okf_agent/docs.py` (new): render deterministic, human-readable reference Markdown and implement build/check operations.
- `okf_agent/cli.py`: add nested `docs build` and `docs check` commands with a configurable repository path and output directory.
- `okf_agent/validation.py`: validate generated marker pairing and local links for generated reference pages, while keeping existing result keys stable.
- `tests/test_docs.py` (new): cover index-backed output, preservation, deterministic builds, check behavior, and on-demand boundaries.
- `tests/test_lifecycle.py`: cover any index reader/build metadata changes needed by docs.
- `README.md`: document the human-facing workflow and the difference between `.okf/`, `.okf-index/`, and `/docs`.

## Interfaces

- `discovery.indexed_content(root: Path) -> dict[str, list[dict[str, str]]]` returns sorted `documents` (`path`, `content`), `symbols` (`name`, `path`), and `edges` (`source`, `target`, `kind`) from the existing SQLite index. It raises a clear `FileNotFoundError` when no index exists and does not write to the index.
- `docs.build_docs(root: Path, output: Path | None = None) -> dict[str, object]` refreshes the index with `build_index(root)`, renders the known reference pages, preserves text outside OKF markers, writes only changed files, and returns output paths plus an `updated` list.
- `docs.check_docs(root: Path, output: Path | None = None) -> dict[str, object]` renders expected pages from a freshly built in-memory/index snapshot without changing documentation files. It returns `ok`, `stale`, and `errors`; stale pages make the CLI exit nonzero.
- Default output directory: `<root>/docs/reference/`. The only machine-managed files in this feature are `docs/reference/index.md` and `docs/reference/code-map.md`.
- Generated pages contain paired `<!-- okf:generated:start -->` and `<!-- okf:generated:end -->` markers. Text outside these markers is curated and retained. The generator does not delete arbitrary files or directories.

## Implementation Plan

### Task 1: Read the existing index through a stable interface

**Files:** `okf_agent/discovery.py`, `tests/test_lifecycle.py`

- [x] **RED:** Write `test_indexed_content_returns_sorted_documents_symbols_and_edges` and `test_indexed_content_missing_index_is_read_only`; run them and confirm the expected missing-function failures.
- [x] Implement `indexed_content(root: Path) -> dict[str, list[dict[str, str]]]` using parameter-free `SELECT` statements and close the SQLite connection in `finally`.
- [x] **GREEN:** Run `python -m unittest tests.test_lifecycle -v`; the new tests and existing lifecycle tests pass.
- [x] Run `python -m unittest tests.test_lifecycle -v`; all lifecycle and index-reader tests pass.

### Task 2: Render deterministic human-facing reference pages

**Files:** `okf_agent/docs.py` (new), `tests/test_docs.py` (new)

- [x] **RED:** Write `test_rendered_pages_link_curated_overview_and_indexed_code`, `test_rendered_pages_do_not_infer_behavior`, and `test_rendered_pages_handle_empty_index`; run them and confirm renderer behavior is missing.
- [x] Implement pure render functions that accept indexed rows and repository facts and return a mapping of relative page path to Markdown string; sort paths, symbols, and edges before rendering.
- [x] Add links relative to `docs/reference/` (for example, `../../okf_agent/cli.py`) and escape Markdown link destinations/titles safely.
- [x] **GREEN:** Run `python -m unittest tests.test_docs -v`; page content, links, bounded claims, and empty-index behavior pass.

### Task 3: Build pages on explicit request while preserving curated content

**Files:** `okf_agent/docs.py`, `okf_agent/knowledge.py`, `tests/test_docs.py`

- [x] **RED:** Write `test_build_docs_creates_only_reference_pages`, `test_build_docs_preserves_curated_text_outside_markers`, `test_build_docs_leaves_unrelated_docs_untouched`, `test_build_docs_is_byte_deterministic`, and `test_build_docs_rejects_page_symlink_outside_repository`; run them and observe the expected missing-build behavior.
- [x] Implement `build_docs(root, output=None)` to call `build_index(root)`, render pages, insert/replace only the generated block, and write files only when bytes differ.
- [x] **GREEN:** Run `python -m unittest tests.test_docs -v`; build, preservation, output-boundary, and repeatability tests pass.

### Task 4: Detect stale docs without rewriting them

**Files:** `okf_agent/docs.py`, `tests/test_docs.py`

- [x] **RED:** Write `test_check_docs_detects_missing_and_stale_pages` and `test_check_docs_does_not_write_docs_or_persistent_index`; confirm check behavior is absent.
- [x] Implement `check_docs(root, output=None)` by building a disposable index in a temporary directory, rendering the expected generated blocks, and comparing them with existing pages. Do not call `build_docs()` or rewrite the repository's `.okf-index/`.
- [x] Return actionable stale-page diagnostics that name each relative path and recommend `okf-agent docs build`.
- [x] **GREEN:** Run `python -m unittest tests.test_docs -v`; missing/stale detection is correct and check mode is read-only.

### Task 5: Expose `docs build` and `docs check` in the CLI

**Files:** `okf_agent/cli.py`, `tests/test_docs.py`

- [x] **RED:** Write `test_docs_cli_build_and_check_exit_codes`, `test_docs_cli_resolves_output_within_repository`, and `test_docs_help_lists_subcommands`; verify the currently unsupported CLI behavior fails as expected.
- [x] Add `--output` to both commands; resolve relative output paths against the selected repository root and reject paths that escape the root.
- [x] Implement nested subcommands without changing existing command syntax or exit codes.
- [x] **GREEN:** Run `python -m unittest tests.test_docs tests.test_onboarding -v`; new commands work and existing CLI behavior remains intact.

### Task 6: Prove generation stays opt-in and validate the output

**Files:** `okf_agent/validation.py`, `tests/test_docs.py`, `tests/test_onboarding.py`

- [x] **RED:** Write `test_lifecycle_commands_do_not_generate_docs` and `test_validate_rejects_malformed_generated_markers_and_broken_links`; observe whether current commands/validator violate those expectations.
- [x] Extend validation without changing its return keys (`ok`, `errors`, `warnings`); keep diagnostics repository-relative.
- [x] **GREEN:** Run `python -m unittest tests.test_docs tests.test_onboarding tests.test_lifecycle -v`; opt-in and validation checks pass.

### Task 7: Document the user workflow and operational boundary

**Files:** `README.md`, `docs/superpowers/plans/2026-10-04-reliable-repository-knowledge.md`

- [x] Update the README with `okf-agent docs build` and `okf-agent docs check`, default output paths, and a short explanation of indexed facts versus curated explanations.
- [x] Amend the existing broader plan's Task 7 to make the SQLite index an explicit input, preserve curated sections, require on-demand generation, and keep `/docs` writes out of hooks and normal update flows.
- [x] Confirm existing examples and setup commands still reflect the actual CLI.
- [x] Run `python -m unittest discover -s tests -v`; all existing and new tests pass.
- [x] Run `okf-agent validate .`; it returns `ok: true`.

## Completion Criteria

- A user can run `okf-agent docs build` and receive a small, linked, human-readable reference under `docs/reference/`.
- A user can run `okf-agent docs check` to learn whether those generated sections are stale, without rewriting docs or the repository's persistent index.
- Re-running build on unchanged inputs produces no diff.
- Hand-authored text outside generated blocks and unrelated `/docs` files are preserved.
- No hook, watcher, initialization, update, or ordinary agent run generates human-facing docs.
- Documentation claims are constrained to index data and curated `.okf/` source material.
