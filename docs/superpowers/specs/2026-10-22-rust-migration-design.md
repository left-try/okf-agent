# OKF Agent Rust Migration Design

## Goal

Move OKF Agent's implementation toward Rust using the workspace and boundary patterns from [`left-try/mae`](https://github.com/left-try/mae), while keeping Python available during a staged transition. Preserve the existing user-facing commands, supported workflows, on-disk OKF format, and compatibility entry points as Rust implementations become ready. Improve runtime and maintenance costs where measured evidence supports the work.

## Current System

The current package is a Python CLI (`okf_agent`) with these capability areas:

- Repository setup, root discovery, adapter installation, lifecycle updates, validation, doctor checks, client launching, and a file watcher.
- Source and manifest discovery, symbol/import extraction, SQLite FTS indexing, and search.
- Standalone Markdown bundle indexing, YAML frontmatter parsing, validation, search, graph queries, and JSON export.
- Generated documentation, stale checks, and a local read-only documentation server.
- Workflow policy parsing, validation, profile installation, and risk/method resolution.
- Optional repository and bundle MCP servers.
- A Python package entry point installed as `okf-agent`.

The repository treats `.okf/` as curated source material and `.okf-index/` as disposable generated data. The SQLite tables and MCP/CLI JSON responses are consumed outside individual modules and should be treated as compatibility contracts. Current repository tests provide a behavioral inventory, but this design does not authorize changing or running them.

## Reference From MAE

Adopt MAE's separation between domain, application, adapters, verification boundaries, and CLI. Do not copy its React-to-Slint migration-specific adapters, frontend fixture, or unrelated OPF/AAMF systems. OKF Agent's target boundaries should reflect repository knowledge, source indexing, bundle indexing, workflows, and client integrations.

## Proposed Workspace

Use a Cargo workspace with small crates and one-way dependencies:

```text
crates/
  okf-domain/       # Validated domain values, policy models, diagnostics, contracts
  okf-application/  # Use cases: index, search, update, validate, workflow resolution
  okf-adapter-fs/   # Safe filesystem access, path policy, templates, generated blocks
  okf-adapter-sqlite/ # Disposable repository and bundle indexes, migrations, FTS
  okf-adapter-git/  # Repository discovery and changed-file integration
  okf-adapter-mcp/  # Optional MCP transport, isolated from core use cases
  okf-cli/          # Command parsing, JSON/text output, process launch and exit codes
```

The domain crate must not depend on filesystem, SQLite, MCP, or command-line libraries. Application use cases depend on domain contracts. Adapters implement those contracts. CLI and MCP are delivery mechanisms over the same application APIs. Crates may be combined if implementation evidence shows a boundary adds overhead without useful isolation; public contracts remain stable.

## Migration Strategy

1. **Record compatibility contracts.** Map every current command, arguments, defaults, output shape, exit status, file write, and MCP tool into a migration inventory. Capture representative behavior from the existing Python implementation before replacing a capability.
2. **Create the Rust workspace and shared domain model.** Establish errors, diagnostics, path handling rules, and versioned index interfaces before moving behavior.
3. **Port deterministic core capabilities first.** Start with workflow policy parsing/resolution and validation primitives, then repository discovery and search/index operations, then bundle operations. Use small use cases with injected adapters so logic can be exercised independently.
4. **Port filesystem and lifecycle behavior.** Implement initialization, generated-block preservation, lifecycle updates, docs build/check/server, enforcement installation, and watcher/process behavior in dependency order. Protect symlink and repository-boundary behavior.
5. **Add Rust delivery adapters.** Add the Rust CLI and optional MCP adapter over the shared application layer. Keep process launching and MCP transport outside domain/application logic.
6. **Keep Python compatibility.** Keep the Python package, `okf-agent` command, and existing module-level public behavior available while parity is incomplete. During transition, route a capability to one canonical implementation at a time; avoid two implementations independently writing the same repository state. The Python facade may delegate to the Rust executable once packaging and failure behavior are reliable, while preserving Python imports that are part of the compatibility surface.
7. **Retire duplication only after a separate compatibility decision.** Do not remove the Python package or change installation expectations as part of the initial Rust port. Any eventual deprecation requires a separate decision based on adoption and packaging experience.

## Data and Compatibility Contracts

- Preserve `.okf/` directory layout, frontmatter/manifest semantics, curated text, and generated-marker ownership rules.
- Keep `.okf-index/` disposable and rebuildable. Preserve the current SQLite schema and FTS behavior initially. Any schema change must be versioned, provide a safe rebuild path, and avoid corrupting an index after interruption.
- Keep CLI command names, flags, defaults, JSON field names, and exit-code meanings stable unless a change is explicitly approved.
- Keep MCP tool names, argument names/defaults, and result shapes stable. MCP remains optional and must not become a dependency of the core CLI.
- Keep bundle indexing read-only with respect to source Markdown; its explicit index directory remains the only generated bundle output.
- Preserve the Python package entry point and documented installation flow during staged migration. Distribution of Rust binaries through Python packaging must be resolved before the Python CLI delegates to Rust by default.
- Maintain platform-aware behavior for Windows, macOS, and Linux, especially path normalization, symlinks, process launching, Git hooks, and watcher behavior.

## Performance and Reliability

Do not assume that a Rust rewrite alone is an optimization. Establish representative measurements for cold full indexing, repeated unchanged indexing, FTS search, bundle graph queries, and memory use on small and larger repositories. Keep full rebuild as the correctness baseline. Add incremental indexing, parallel file reads, or caching only when measurements show a meaningful improvement and parity checks show equivalent results.

Any incremental index must detect file additions, deletions, renames, and content changes; recover from stale or interrupted state; and support a clean rebuild. Deterministic outputs and stable ordering are preferred. Avoid holding an entire large corpus in memory when streaming or bounded batches are straightforward.

## Error Handling and Safety

- Return structured diagnostics from application use cases; let CLI/MCP adapters render them appropriately.
- Keep malformed user configuration distinguishable from I/O, index, and internal errors.
- Resolve repository-relative paths and enforce repository/output boundaries before writes.
- Preserve curated sections in `.okf/` and agent instruction files; only replace explicitly machine-owned generated regions.
- Do not overwrite non-OKF Git hooks or follow symlinks outside approved roots when writing generated output.
- Keep `docs check` and bundle validation read-only with respect to source and persistent generated state.

## Completion Criteria

- All listed capability areas have Rust implementations behind MAE-style boundaries.
- The Python package and existing CLI remain usable through the transition, with command behavior and external contracts preserved.
- Rust CLI and MCP adapters, when present, call the same application use cases rather than duplicating business logic.
- `.okf/` curated content, SQLite rebuildability, and source bundle immutability remain intact.
- Performance work is supported by repeatable before/after measurements, and any incremental path can fall back to a clean rebuild.
- Python deprecation or removal is explicitly out of scope for this migration design.

## Implementation Plan Boundary

The detailed implementation plan should split the migration into reviewable phases. Each phase should have an explicit capability inventory, crate/file ownership, compatibility contracts, rollout behavior, and acceptance conditions. The initial implementation phase should establish the workspace and port one low-risk vertical capability end-to-end before expanding. Package publishing, binary distribution, and any Python deprecation decision must be handled as explicit phase decisions rather than hidden consequences of the port.
