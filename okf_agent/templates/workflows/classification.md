# Task Classification and Risk

Choose one primary type by intent; add secondary types when useful:

- feature: introduce or change intended behavior.
- bugfix: restore intended behavior.
- hotfix: urgently repair or mitigate an incident.
- docs: explanatory prose, examples, or reference material.
- tests: coverage, fixtures, or test infrastructure.
- refactor: preserve behavior while restructuring.
- maintenance: dependency, build, or operational upkeep.

Assess rigor separately as light, standard, or strict. Consider ambiguity, affected components, statefulness, security/data impact, reversibility, production exposure, and test-oracle strength. A feature does not imply GTDD; urgency does not waive critical verification.
