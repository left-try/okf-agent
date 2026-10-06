# Adaptive Workflow Policy Design

## Goal

Add optional, repository-owned development workflow policy to `okf-agent`. A coding agent classifies a task, assesses risk, and selects a proportional combination of SDD, TDD, and (when supported) GTDD. The policy is resolved consistently through CLI and MCP, and detailed workflow guidance is loaded only when relevant.

This design covers the first usable release (policy, optional initialization, adapters, validation, docs and MCP/CLI). Independent GTDD orchestration is a separate future execution backend.

## Existing Constraints

- Python 3.11+ with no mandatory runtime dependencies.
- `.okf/manifest.yaml` remains schema version 1.
- Workflow profiles remain optional; old repositories without them continue to initialize and validate.
- Existing curated files and non-OKF Git hooks must be preserved.
- Native adapters must stay concise and must not claim that computed context is injected into launched clients.
- OKF knowledge validation is separate from application test evidence.

## Policy Model

Task categories are `feature`, `bugfix`, `hotfix`, `docs`, `tests`, `refactor`, and `maintenance`. An assessment has one primary type and zero or more secondary types. Classification is based on intent, not filenames. `refactor/maintenance` may be accepted as an input alias only if normalized to a canonical type.

Risk is resolved separately as `light`, `standard`, or `strict`. The caller provides semantic assessment; the resolver does not infer arbitrary task meaning from prose. Inputs include task types, requested risk, a high-impact flag, optional requested methods, and execution capabilities. High-impact assessments force strict rigor. Explicit task choices override repository defaults; repository policy overrides built-in defaults. Capability limits remain visible in the decision.

The result is deterministic structured data containing normalized task types, effective rigor and methods, ordered deduplicated phases, reasons, artifacts/gates, execution mode, missing capabilities, and fallback rationale. High-impact/strict assessments cannot silently become light. Hotfix urgency never waives critical verification. Explicit method choices and required methods for strict, ambiguous, or behavioral work remain effective even if their optional profile document is not enabled; the available profile list remains separately visible through policy.

SDD clarifies the contract; TDD develops against meaningful executable tests; GTDD adds separately executed coder/tester/auditor roles and controlled handoffs. Until an orchestration executor exists, GTDD resolves to SDD/TDD as appropriate plus explicitly labeled sequential adversarial review, with `execution: single-agent` and a capability limitation. The resolver never claims independent GTDD in that mode.

## Default Policy

Defaults are candidates that the resolver adapts to the assessment, not unconditional obligations:

| Primary type | Candidate methods | Default rigor |
| --- | --- | --- |
| feature | SDD, TDD | standard |
| bugfix | TDD | standard |
| hotfix | TDD | standard |
| docs | none | light |
| tests | none | light |
| refactor | none | standard |
| maintenance | none | light |

With no per-task risk supplied, configured `default_rigor` applies to behavioral categories (`feature`, `bugfix`, `hotfix`, `refactor`); documentation, tests-only, and maintenance tasks retain their light category default unless repository default rigor is strict, which raises all categories. Explicit high-impact assessment forces strict rigor, and a hotfix cannot be below standard. Strict risk adds an explicit contract/invariant and broader verification phases. Ambiguous behavior adds compact SDD. Behavioral changes categorized as docs/tests include TDD even if no method flag is supplied. A task with no meaningful executable behavior may omit TDD. GTDD is eligible only when the task benefits materially from independent challenge and execution capabilities include distinct role contexts and enforceable handoffs.

## Configuration Contract

Store policy in optional `.okf/workflows/config.json`, parsed with the standard library. Version 1 contains `schema_version`, `enabled_profiles`, `selection`, `default_rigor`, `defaults`, and GTDD `execution`/`fallback` settings. Reject unknown keys, unsupported schema versions, invalid enums, malformed references, and incompatible settings with actionable errors. Keep `manifest.yaml` unchanged.

The initial configuration enables the `sdd`, `tdd`, and `gtdd` profiles, uses adaptive selection and standard default rigor, and sets GTDD to `recommend-only` with `sequential-adversarial-review` fallback. Installed profiles do not imply available execution capabilities.

## Repository Files and Initialization

Optional repository-owned files:

```text
.okf/workflows/config.json
.okf/workflows/active.md
.okf/workflows/classification.md
.okf/workflows/profiles/{sdd,tdd,gtdd}.md
.okf/workflows/roles/{coder,tester,auditor}.md
```

Package starter templates as package data. Existing `okf-agent init .` remains compatible and does not implicitly reset an installed policy. An explicit `--profiles sdd,tdd,gtdd` opts into initialization. Installing the selected policy happens before index and wiki generation. Repeated initialization is idempotent; curated existing profile files are retained and conflicts are reported. Existing repositories without profiles continue to validate.

`.okf/workflows/active.md` is stable policy routing, not a record of the latest task. Transient run logs and concurrent task state are out of scope. Do not make `.okf/changes/current.md` authoritative for task classification.

## Interfaces

Proposed CLI:

```text
okf-agent init . [--profiles sdd,tdd,gtdd]
okf-agent workflow show .
okf-agent workflow explain . --type bugfix --risk standard
okf-agent workflow resolve . --type feature --risk strict --stateful --execution single-agent
```

`show` and `explain` are read-only. `resolve` accepts an explicit assessment and is read-only unless a future explicit persistence option is added. Commands return structured JSON with reasons, ordered phases, artifacts/gates, and capability limitations. There is no mandatory interactive prompt.

Expose `okf.get_workflow_policy` and `okf.resolve_workflow` MCP tools using the same parser and resolver as CLI. Extend `okf.get_change_context` with a short workflow routing entry when optional profiles exist; do not preload all profiles or role documents.

## Native Adapters

Keep AGENTS, Claude, Cursor and Gemini adapters short: retrieve relevant OKF context; read `active.md` if present; classify and select only relevant profiles/roles; update knowledge and validate at completion. Use one policy source of truth. Apply the same installation path from `init` and `run`.

Introduce clearly delimited workflow routing in adapter instructions and migrate the existing legacy OKF protocol safely. Installation is idempotent, preserves curated surrounding text and unrelated hook content, and reports malformed or conflicting marker layouts instead of deleting user content. Do not claim that `run` injects context when it only emits a payload.

## Validation and Documentation

Validate optional config schema and enabled profile/role references with actionable file paths. Repositories without workflow configuration remain valid. Include policy configuration in wiki freshness inputs if edits to JSON could otherwise leave the generated wiki apparently current. Mirror workflow Markdown using the existing docs behavior. Preserve workflow-owned curated material during normal refreshes. Validation confirms knowledge integrity, not whether application tests passed.

Document task taxonomy, risk dimensions, method composition, defaults, capability fallback, CLI usage, initialization and realistic lightweight/strict examples. Avoid claims of fidelity to a particular GTDD paper.

## Workflow Resolution Phases

Resolve an ordered, deduplicated plan:

1. Retrieve relevant repository context.
2. Classify work and assess risk.
3. Specify behavior when needed.
4. Establish verification evidence.
5. Implement.
6. Add adversarial testing when selected and supported; otherwise record the single-agent fallback.
7. Review results and unresolved issues.
8. Update repository knowledge and validate.
9. Report evidence and limitations.

Shared obligations such as SDD acceptance criteria and TDD expectations belong to one contract phase, not duplicated instructions.

## Out of Scope

- Independent GTDD orchestration, executor SDK integration, isolated role worktrees, run state machines, role write enforcement, resume/cancel/budget controls, and revision-pinned audit records.
- Automatic semantic classification from free-form prose.
- Durable task records or task-specific edits to `current.md`.
- Changing the existing manifest schema or adding a mandatory dependency.

## Acceptance Criteria

1. The same assessment and policy always produce the same decision.
2. Classification, rigor, selected methods, high-impact assessment, and available execution capability remain separate in API results.
3. Strict/high-impact work cannot resolve to light rigor; hotfix urgency cannot bypass verification.
4. Mixed task types preserve one primary type and optional secondary types.
5. GTDD eligibility requires both material need and independent execution capability; unavailable execution is explicit and accurately falls back.
6. Invalid configuration (including unknown keys/version) fails with actionable diagnostics.
7. Initialization remains compatible, optional, idempotent, and non-destructive to curated files.
8. CLI and MCP call the same resolver and return equivalent decisions.
9. Legacy adapter installation can be upgraded idempotently without overwriting curated content; malformed conflicts are reported.
10. Repositories without profiles continue to initialize and validate.
11. Wiki freshness accounts for workflow config, and workflow Markdown appears in generated docs.
12. Existing lifecycle, onboarding, docs, web, CLI and MCP behavior remains covered by relevant regression tests.
