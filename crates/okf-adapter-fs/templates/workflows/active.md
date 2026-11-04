# Active Development Workflow

Classify the task by intent and assess risk separately. Select only the smallest sufficient workflow profiles listed by .okf/workflows/config.json; do not load every profile or role.

Use specification-driven development (SDD) when behavior, contracts, or edge cases need clarification. Use test-driven development (TDD) for meaningful executable behavior. Consider GTDD only when independent challenge materially helps and separate role contexts with enforceable handoffs are available.

When independent execution is unavailable, use and report a single-agent adversarial review. Installing GTDD guidance does not create independent execution. Preserve OKF's retrieve → change → update → validate lifecycle.
