# Active Development Workflow

Classify the task by intent and assess risk separately. Select only the smallest sufficient workflow profiles listed by .okf/workflows/config.json; do not load every profile or role.

Follow strict workflow quality rules:
- **Specification-Driven Development (SDD)**: Mandatory contract definition with ADR/rationale, strict interfaces, Mermaid diagrams (`sequenceDiagram` / `stateDiagram`), and layer Call Trace.
- **Test-Driven Development (TDD)**: Mandatory Red-Green-Refactor sequence. Write failing test first, confirm failure reason, make minimal green implementation, and refactor without weakening tests.
- **Cognitive Debt Gates**: Validate diff budgets (300-400 LOC), zero escape hatches, zero test weakening, and architectural boundary rules via `okf-agent gate .`.
- **GTDD**: Consider GTDD only when independent challenge materially helps and separate role contexts with enforceable handoffs are available.

When independent execution is unavailable, use and report a single-agent adversarial review. Installing GTDD guidance does not create independent execution. Preserve OKF's retrieve → change → update → validate lifecycle.
