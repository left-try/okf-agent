# Development

Workflow guidance is optional and documented in README.md. Install it explicitly with okf-agent init . --profiles sdd,tdd,gtdd. Classify work by intent and assess rigor separately; use SDD/TDD proportionally. GTDD is independent only when separate role contexts and enforceable handoffs are available; otherwise report a single-agent adversarial review. okf-agent validate checks knowledge integrity, not application test results.

Quality gates and pre-commit automation can be checked locally with:
- `okf-agent update . --changed-files <files>`
- `okf-agent validate .`
- `okf-agent gate .`
- `pre-commit run --all-files`

<!-- okf:generated:start -->
<!-- okf:generated:end -->
