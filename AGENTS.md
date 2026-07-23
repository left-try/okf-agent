# OKF Agent Protocol

Before planning or editing, call `okf.get_change_context` (or `okf-agent index` then inspect `.okf/`). After code changes, call `okf.update_after_change` or `okf-agent update`. Before completion, call `okf.validate` or `okf-agent validate`. Preserve curated sections in `.okf/`; only generated blocks are machine-owned.

## OKF Repository Protocol

Before planning or editing, run `okf-agent index .` when the index is absent, then retrieve repository context. After source changes, run `okf-agent update . --changed-files <changed paths>`. Before completion, run `okf-agent validate .`. Preserve curated content outside OKF-generated blocks.
