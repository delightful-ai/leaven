# Leaven Philosophy Compliance Cleanup

This is the durable audit and cleanup contract for keeping the current Leaven
workspace aligned with the repo philosophy docs:

- `docs/philosophy/type_design.md`
- `docs/philosophy/trait_design.md`
- `docs/philosophy/error_design.md`
- `docs/philosophy/test_design.md`
- `AGENTS.md` topology rules

The goal is not to make the code merely pass. The goal is to keep looping until
the public library surface is clear, testable, and honest.

## Serious-Drift Standard

This document should track findings that can actually damage library
maintainability:

- spec/code disagreements that make a public API lie about what exists;
- invalid public states that can change behavior or bypass enforcement;
- topology leaks that let the wrong crate know the wrong fact;
- public traits or erased adapters without contract tests;
- fallible paths that hide caller decisions, durable graph errors, or source
  chains;
- coverage or tooling gaps that let those regressions land unnoticed.

Do not add low-signal checklist findings just because a type has a string field,
an enum has a TODO, or a skeleton crate reserves a future name. Skeletons are
only findings when they expose behavior that can mislead a caller or rot a
boundary.

## Audit Axes

### Type Design

Every public type in behavior-bearing crates should answer:

- What invalid states can this type currently express?
- Are causal/provenance facts preserved as typed data rather than metadata or
  strings?
- Are numeric values finite and domain-valid by construction?
- Are string fields open-world identifiers by design, or are they hiding a
  closed set that should be an enum/newtype?

### Trait Design

Every public trait should answer:

- What capability does this trait own?
- What laws would make an implementation wrong?
- Is there a static path and, when exposed, a tested object-safe path?
- Does a default method follow from the trait laws, rather than convenience?
- Is an empty marker trait being used as a placeholder for a real capability?

### Error Design

Every fallible public operation should answer:

- Does the error enum preserve the caller's decision surface?
- Is source/debug information preserved until a deliberate boundary?
- Are durable `RunEvent::Error` records emitted when graph-visible work is
  refused?
- Are string errors confined to true outer edges or generic human messages?

### Test Design

Every test should be one of:

- Law
- Example
- Scenario
- Regression

Every test should kill a plausible bad implementation. Trait surfaces need
contract tests once the trait is public and behavior-bearing.

## Fixed In This Pass

- Evaluation/proposal/optimizer/store failures now use `ErrorRecord::from_error`
  at durable event boundaries, preserving debug/source chains.
- Evidence-store failure after evaluation request recording now emits a durable
  stage error and leaves assessment mutation incomplete.
- `ScalarEvidence` is finite-by-construction and refuses `NaN` and infinities.
- `Amount`, `Cost`, and `Budget` no longer expose raw seconds `f64` values.
  Amount-like values are finite and non-negative by construction, serde
  deserialization preserves the same invariant, and budget tests cover the
  former `NaN` bypass shape.
- `leaven-derive` no longer exposes derive macros that silently expand to
  nothing. Reserved derives now fail explicitly, with `trybuild` coverage, until
  the real derive contracts are implemented.
- UUID-backed ID macro generation now accepts doc attributes, so public
  identifier docstrings do not break the typed-ID implementation.
- Static-to-dynamic stage adapters for proposer, evaluator, preference, and
  stopper have explicit contract tests.
- `DynRenderer` is not exposed as an empty marker trait. The planned surface is
  preserved as a TODO until the erased value/target/view contract exists.
- `RunPersistence` moved out of `engine.rs` into a named persistence capability
  module and now returns a structured `RunPersistenceError`.
- Public/external surfaces touched in this pass have docstrings.

## Open Findings

### Renderer Erasure

`DynRenderer` remains planned but intentionally absent from code.

Required cleanup:

- Define the erased value/target/view contract.
- Add stage trait contract tests.
- Only then expose `DynRenderer`.

### Derive Macro Implementation

`leaven-derive` now fails explicitly instead of silently expanding to nothing,
but the spec-defined derive implementation is still open.

Required cleanup:

- Implement `Artifact`, `ContentAddressed`, and `EditSurface` derives according
  to the topology spec.
- Add passing and failing `trybuild` suites for generated impl shape,
  `content_skip`, unsupported items, and error messages.
- Keep proc-macro entrypoints in `lib.rs` as thin delegates; parsing and codegen
  belong in named modules.

### Stringly External References

`ExternalRef { kind: String, id: String }` is intentionally open-world today,
but this should stay under review. If the set of external reference kinds
becomes closed or behavior-bearing, it should become a typed enum/newtype pair.

### Unsupported Evaluation Sets

`EvaluationResolveError::UnsupportedSet(String)` currently carries a string.
If unsupported-set behavior grows beyond tagged sets, replace the string with a
typed unsupported-set descriptor.

## Completion Bar

This audit is not complete until:

- Every open finding above is fixed or has an explicit milestone-level deferral
  with a reason.
- `docs/testing/README.md` names any new suites added by the cleanup.
- `just check` passes without lowering `coverage_line_floor`.
