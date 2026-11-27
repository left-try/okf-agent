# Architecture

<!-- okf:generated:start -->
## Generated facts
- Manifests: Cargo.toml
<!-- okf:generated:end -->

## Curated constraints
- `okf-domain`: Pure domain models, graph algorithms (`petgraph`), gate policies. Must not depend on any adapter or application layer.
- `okf-application`: Use cases, ports, and lifecycle orchestration.
- `okf-adapter-sqlite`: Embedded SQLite storage for FTS5 BM25 search and source code symbol/import persistence.
- `okf-adapter-fs`: File system discovery, template rendering, and boundary configuration loading.
- `okf-adapter-git`: Git command wrapper for diff and status detection.
- `okf-adapter-mcp`: Model Context Protocol server exposing OKF tools to coding agents.
- `okf-cli`: CLI binary with clap commands.
