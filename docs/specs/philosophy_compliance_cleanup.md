# Leaven Philosophy Cleanup Ledger

This is the current cleanup ledger for philosophy-driven maintainability work.
Use it only for issues that can damage the public library surface or crate
boundaries. General design pressure lives in `docs/philosophy/*`; completed
cleanup history belongs in `jj log`, reviews, or dated plans.

Track a finding here only when it names a real current risk:

- a spec/code disagreement that makes a public API lie;
- an invalid public state that can bypass enforcement;
- a topology leak that lets the wrong crate know the wrong fact;
- a public trait or erased adapter without contract tests;
- a fallible path that hides caller decisions, durable graph errors, or source
  chains;
- a tooling gap that can let those regressions land unnoticed.

Do not use this file for low-signal checklist debt. Skeletons, TODOs, and
reserved names are findings only when they expose behavior that can mislead a
caller or rot an ownership boundary.

## Open Findings

### Renderer Erasure

`DynRenderer` remains planned but absent from code. Do not expose it until the
erased value/target/view contract exists and stage trait contract tests prove
both the static and object-safe paths.

### Derive Macro Implementation

`leaven-derive` is not a current workspace crate. Reintroduce it only with real
`Artifact`, `ContentAddressed`, and `EditSurface` codegen, passing and failing
`trybuild` suites, topology coverage, and public route maturity classification.

### Probe Evaluation Handles

`EvalHandle` and `ProbeRecorder` are public, non-constructible engine
reservations. They must stay inert until the evaluator registry, permission
algebra, budget use, durable graph tagging, and population eligibility rules for
probe evaluations are specified and tested.

### Stringly External References

`ExternalRef { kind: String, id: String }` is intentionally open-world today. If
external reference kinds become closed or behavior-bearing, move the shape to a
typed enum/newtype pair and update downstream reflection/provenance tests.

## Completion Bar

This ledger is not empty until every open finding above is implemented or moved
to an explicit milestone-level deferral with a named owner and reason.

Completion for behavior changes still requires the owning crate tests and
`just check`; do not lower coverage floors to land cleanup work.
