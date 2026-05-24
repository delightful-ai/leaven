# 2026-05-24 Call-Authority Execution Redaction Error Review

Reviewer: Kepler (`019e5b40-92f5-7dd3-9f8f-d2b4b2b384de`)

Scope:

- `crates/leaven-public-seam/src/error.rs`
- `crates/leaven-public-seam/src/call_authority.rs`
- `crates/leaven-public-seam/tests/plan_document.rs`
- Matrix row: `ps1.visibility.data_class_propagation`

Prompt notes:

- Passing the parent-run tests was explicitly not sufficient sign-off.
- Review focus was spec drift between Rust errors and Plan Result redaction
  wire objects, fake passes, missing negative tests, topology leaks, and
  public-maturity overclaiming.

Verdict:

- No blocking findings for adding narrow partial evidence to
  `ps1.visibility.data_class_propagation`.
- The row must remain pending.

Findings:

- `PublicSeamError::CallAuthorityDenied` stays on the Rust/package error
  surface and does not pretend to be a locked Plan Result `Redaction` wire
  object. The representative harness still emits top-level Plan Result
  `redactions: []`.
- The literal dependency-class test rejects the row's named fake pass because
  the denied `external.secret` class is attached to a literal expression, stored
  as binding metadata, and checked before host call execution rather than read
  from the initial case.
- Existing nearby tests cover case-query dependency drops, nested agent command
  classes, graph rows, workspace listing entries, and domain JSON false
  positives, so the new literal side-channel assertion is sufficient for this
  narrow tranche.
- No engine, runtime, or provider behavior leaked into the diff.

Residual limitations:

- `PublicSeamError::CallAuthorityDenied.redactions` carries data-class names,
  not full Plan Result `Redaction` wire objects.
- This is not ACP/provider runtime behavior.
- This is not full engine/evidence/all-route data-class propagation.
- This does not promote `ps1.visibility.data_class_propagation` beyond pending
  partial evidence.

Approved evidence wording:

Additional partial evidence that representative capability-scoped plan
execution preserves call-authority data-class redaction facts in a structured
Rust error when dependency-side classes are denied before any host call runs.
