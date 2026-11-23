# Test-Driven Development

For meaningful executable behavior, establish a relevant failing test, confirm it fails for the intended reason, implement the smallest coherent change, then rerun the targeted and relevant regression checks. Prefer regression-first coverage for defects. Do not weaken valid tests or claim test-first work unless the test preceded implementation.

## Mandatory TDD Red-Green-Refactor Lifecycle

1. **RED (Failing Test First)**:
   Write an externally observable test exercising the new requirement or reproduced bug before touching production code. Execute the test suite and verify that it fails specifically for the expected missing capability or defect assertion.

2. **GREEN (Minimal Coherent Implementation)**:
   Write only the minimal necessary implementation to satisfy the failing test and verify all tests turn green. Avoid pre-emptive complexity or unrelated changes.

3. **REFACTOR (Preserve Invariants & Quality)**:
   Refactor clean code under green tests. Ensure cognitive debt gates pass (`okf-agent gate .`):
   - Zero test weakening (never reduce assertions or remove test cases).
   - Zero escape hatches (`TODO`, `FIXME`, `unimplemented!`, `todo!`, `NotImplementedError`, `: any`, `as any`, `@ts-ignore`, `typing.Any`).
   - Diff budget adherence (target 300-400 LOC max).
