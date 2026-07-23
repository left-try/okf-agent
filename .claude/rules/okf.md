# Repository Instructions

## OKF Repository Protocol

Before planning or editing, run `okf-agent index .` when the index is absent, then retrieve repository context. After source changes, run `okf-agent update . --changed-files <changed paths>`. Before completion, run `okf-agent validate .`. Preserve curated content outside OKF-generated blocks.
