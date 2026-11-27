---
name: okf-doc-updater
description: Reviews recent changes and updates corresponding curated documentation in .okf/ before finishing tasks.
---

# OKF Documentation Updater Skill

When completing any coding change or before finishing your task in an OKF-enabled repository:

1. **Check Changed Files**:
   Identify which source files were modified or created.

2. **Inspect Documentation Mappings**:
   - Check `.okf/boundaries.yaml` under `doc_mappings` (or directory structure) to see which `.okf/` pages cover the changed components.
   - For architecture changes, check `.okf/architecture/index.md`.
   - For application/domain changes, check `.okf/application/overview.md`.
   - For operations, check `.okf/operations/development.md`.

3. **Update Curated Prose**:
   - Update descriptions, explanations of new concepts, rationale, and behavioral contracts.
   - Keep machine-generated blocks (`<!-- okf:generated:start -->` ... `<!-- okf:generated:end -->`) untouched or let `okf-agent update` handle them.

4. **Synchronize & Validate**:
   Run:
   ```bash
   okf-agent update . --changed-files <modified files>
   okf-agent validate .
   okf-agent gate .
   ```
