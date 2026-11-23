# Specification-Driven Development

Use SDD when intent, contracts, architecture, or edge cases are unclear or consequential. Record observable behavior, acceptance criteria, inputs/failures, compatibility constraints, invariants, non-goals, assumptions, and verification. Keep a small contract inline; create a durable specification only when it supports implementation, review, or handoff.

## Mandatory SDD Contract Steps

1. **Architecture Decision Record (ADR) & Rationale**:
   Document architectural decisions, tradeoffs, and rationale in `.okf/decisions/` or an inline ADR section. Specify why alternatives were rejected and how invariants are preserved.

2. **Strict Interface & Type Contracts**:
   Define explicit domain invariants, protocol/trait specifications, and concrete input/output types before implementation. Avoid dynamic fallbacks (`Any`, `as any`, `: any`, `typing.Any`).

3. **Mermaid Visual Architecture (sequenceDiagram / stateDiagram)**:
   Include required Mermaid diagrams to visualize control and data flow:
   - `sequenceDiagram`: For protocol handoffs, client-service lifecycles, and cross-layer request-response flows.
   - `stateDiagram`: For stateful components, lifecycle transitions, and error recovery states.

4. **Call Trace & Layer Traversal**:
   Map the explicit call path across boundaries before editing:
   `Caller / Transport` → `Application Entry / Port` → `Domain Aggregate / Model` → `Adapter / Store / External`.
   Verify that all layer boundary rules from `.okf/boundaries.yaml` or `manifest.yaml` are respected.
