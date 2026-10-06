# Adaptive Workflow Policy Implementation Plan

> **For agentic workers:** Implement natively in the requested worktree. Use a test-first cycle for each task and preserve unrelated user changes.

**Goal:** Add optional repository-owned adaptive workflow policy with a shared CLI/MCP resolver, safe profile installation and adapter routing, and policy-aware validation/docs.

**Architecture:** Implement a dependency-free `okf_agent.workflows` module for strict JSON policy loading and deterministic assessment resolution. Integrate optional starter profiles and compact adapter routing, then expose the same functions through CLI and MCP and include workflow policy in validation and wiki freshness.

**Tech Stack:** Python 3.11+, standard library, existing `unittest` suite, setuptools package data.

**Spec:** `docs/superpowers/specs/2026-10-06-adaptive-workflow-policy-design.md`

## Global Constraints

- Keep runtime dependencies empty and `.okf/manifest.yaml` schema version 1.
- Workflow profiles remain opt-in; repositories without them still initialize and validate.
- Preserve curated files, adapter text, and non-OKF hooks.
- CLI and MCP use the same resolver; task semantics are explicitly assessed, not inferred from arbitrary prose.
- Never report independent GTDD when only sequential adversarial review is available.
- Do not commit, push, publish, deploy, or merge.

## Review Focus

- Invalid assessments/configuration need actionable errors (Task 1).
- Strict risk/hotfix cannot weaken verification; GTDD fallback remains truthful (Task 1).
- Reinitialization and adapter migration preserve curated text (Tasks 2–3).
- JSON policy edits invalidate generated wiki freshness (Task 6).
- Existing repositories lacking profiles remain compatible (Tasks 2, 6).

---

### Task 1: Policy Types, Strict Config Loader, and Resolver

**Files:** Create `okf_agent/workflows.py`, `tests/test_workflows.py`.

**Interfaces:** `load_policy(root: Path) -> WorkflowPolicy`; `resolve_workflow(policy: WorkflowPolicy, assessment: WorkflowAssessment, capabilities: WorkflowCapabilities | None = None) -> WorkflowDecision`. Public assessment fields: `primary_type`, `secondary_types`, `risk`, `high_impact`, `requested_methods`, `ambiguous`, `behavioral`, `independent_challenge`. High impact forces strict rigor. Capabilities: `independent_roles`, `enforceable_handoffs`. Decision includes normalized types, rigor, methods, ordered phases, reasons, artifacts/gates, execution mode, missing capabilities, and fallback rationale.

- [x] Write table-driven tests for all task defaults, mixed categories, method overrides, strict/hotfix verification, ambiguous contract selection, behavioral docs/tests, deterministic deduplication, malformed/unknown config fields, and GTDD eligibility/fallback.
- [x] Run `python3 -m unittest tests.test_workflows -v`; first failure confirmed missing `okf_agent.workflows`, then a GTDD capability-path assertion failed for the intended resolver defect.
- [x] Implement strict version-1 config validation and deterministic phase composition with actionable `WorkflowConfigError` messages.
- [x] Rerun targeted tests; 15 passed.

### Task 2: Optional Packaged Profiles and Initialization

**Files:** Create templates under `okf_agent/templates/workflows/{config.json,active.md,classification.md,profiles/*.md,roles/*.md}`; modify `okf_agent/knowledge.py`, `pyproject.toml`, `okf_agent/workflows.py`; test in `tests/test_workflows.py`.

**Interface:** `install_profiles(root: Path, profiles: Sequence[str]) -> ProfileInstallResult`; extend `initialize(root, profiles=None)` without changing no-argument behavior. Existing curated profile files remain; results report created, retained, and conflicting files.

- [x] Test no-profile compatibility, clean/selected install, repeat idempotence, curated file preservation, unknown profiles, and wheel template inclusion.
- [x] Run targeted tests and confirm they fail before implementation.
- [x] Add profile templates, installer, init wiring, and package-data declaration.
- [x] Rerun tests and build a wheel from a clean /tmp copy for package-data verification.

### Task 3: Compact Adapter Routing and Legacy Migration

**Files:** Modify `okf_agent/enforcement.py`; test `tests/test_lifecycle.py`.

- [x] Test routing insertion/idempotence, safe legacy migration, curated surrounding text, malformed marker conflicts, Cursor routing, and preservation of custom hooks.
- [x] Verify tests fail on missing report API and missing new Cursor routing.
- [x] Implement clearly delimited workflow routing, conflict reporting, and additive non-destructive legacy upgrade. Keep `install(root)` backward compatible.
- [x] Rerun targeted tests.

### Task 4: CLI Workflow Commands

**Files:** Modify `okf_agent/cli.py`; test `tests/test_onboarding.py`.

**Interfaces:** `init [path] --profiles ...`; `workflow show [path]`; `workflow explain [path] --type TYPE [--risk RISK] ...`; `workflow resolve [path]` with explicit type/risk/high-impact/method/stateful/execution options. Commands emit JSON, remain read-only, and return nonzero with actionable errors. `run` adds lazy profile routing only when opted in and preserves the honest context-delivery limitation.

- [x] Test commands/JSON, profile-init ordering/default compatibility, GTDD fallback, and unchanged `run --no-launch` reporting.
- [x] Confirm the profiles option initially failed in argparse, then found the empty-`.git` root-discovery issue through a failing regression.
- [x] Implement argparse and wire shared policy functions; `init` and `run` report adapter conflicts.
- [x] Rerun CLI, root-discovery, and onboarding tests.

### Task 5: MCP Resolver Parity and Lazy Context

**Files:** Modify `okf_agent/mcp.py`; test `tests/test_workflows.py` and `tests/test_onboarding.py`.

- [x] Test `okf.get_workflow_policy`, `okf.resolve_workflow`, CLI/MCP result parity, and `context()` routing only when profiles exist without preloading all profile/role contents.
- [x] Confirmed missing context routing and found tuple/list serialization mismatch in parity tests.
- [x] Add MCP tools and short `workflow_routing` context via the shared loader/resolver.
- [x] Rerun MCP tests.

### Task 6: Validation, Wiki Freshness, and Documentation

**Files:** Modify `okf_agent/validation.py`, `okf_agent/docs.py`, `README.md`, `.okf/operations/development.md`; test `tests/test_lifecycle.py`, `tests/test_docs.py`, `tests/test_web.py`.

- [x] Test optional config/reference validation, old repo compatibility, workflow Markdown pages, config-only stale detection, and docs rebuild recovery.
- [x] Confirmed missing profile reference and config-only freshness tests failed before their fixes.
- [x] Validate profiles using the shared workflow policy validator and include `config.json` in wiki source hashes; document taxonomy, defaults, CLI, opt-in and honest fallback.
- [x] Reran docs and lifecycle tests; web tests require loopback sockets unavailable in the current sandbox.

### Task 7: Full Regression and Final Review

- [x] Run `python3 -m unittest discover -s tests -v`; all 91 tests pass, including loopback tests with approved test-only elevation.
- [x] Verify wheel template inclusion from a clean source copy; smoke-check init with and without profiles plus workflow show/explain/resolve.
- [x] Run `python3 -m okf_agent.cli update . --changed-files <actual changed paths>` and `python3 -m okf_agent.cli validate .`; validation returns no errors or warnings.
- [x] Review the complete diff against the acceptance criteria, resolve the adapter parent-symlink path-safety finding with a regression test, and preserve unrelated user changes in the primary checkout.

## Coverage Map

Tasks 1–6 cover spec criteria 1–11 respectively: policy resolution and capabilities; optional initialization/package data; adapter migration; CLI; MCP parity; validation/docs freshness and compatibility. Task 7 covers criteria 12 and performs end-to-end evidence review. Independent orchestration and task-specific transient state are out of scope as defined by the spec.
