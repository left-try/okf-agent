# OKF Agent Protocol

Before planning or editing, call `okf.get_change_context` (or `okf-agent index` then inspect `.okf/`). After code changes, call `okf.update_after_change` or `okf-agent update`. Before completion, call `okf.validate` or `okf-agent validate`. Preserve curated sections in `.okf/`; only generated blocks are machine-owned.
