# 2026-05-24 Call-Authority Redaction Denials Review

Reviewer: Cicero (`019e5b2c-6c4b-7252-a3a6-a901126363b1`)

Scope:

- `crates/leaven-public-seam/src/call_authority.rs`
- `crates/leaven-public-seam/src/lib.rs`
- `crates/leaven-public-seam/src/package.rs`
- `crates/leaven-public-seam/tests/call_authority.rs`
- `crates/leaven-public-seam/AGENTS.md`
- Matrix row: `ps1.visibility.data_class_propagation`

Prompt notes:

- Passing the parent-run tests was explicitly not sufficient sign-off.
- Review focus was spec drift, fake passes, missing negative tests, topology
  leaks, public-maturity overclaiming, and whether the row's named fake pass
  remained possible.

Initial findings and resolutions:

- Reflector target-egress denial was keyed only off capability subject role.
  A capability with a non-reflector subject, a broad LM grant, and a call-local
  `model_role: "reflector"` could carry `case.target`. Resolved by treating an
  LM call as reflector-scoped when either the capability subject role or the
  call-local `model_role` is `reflector`, and by adding a non-reflector-subject
  negative test for that bypass.
- New crate-root exports for `CallAuthorityError`, `CallAuthorityDenial`, and
  `CallAuthorityDenialKind` were not classified in the local public-maturity
  contract. Resolved by classifying them as advanced public seam contracts in
  `crates/leaven-public-seam/AGENTS.md`.

Follow-up verdict:

- No blocking findings for adding narrow partial evidence to
  `ps1.visibility.data_class_propagation`.
- The row must remain pending.

Residual limitations:

- This proves a seam-owned typed denial surface for call-authority data-class
  refusals and representative dependency-aware execution gating.
- It does not prove full monotonic propagation through engine, evidence, ACP,
  provider, or all runtime routes.
- Execution paths that still return `PublicSeamError` collapse the typed denial
  back into the package-level error shape.
- This API returns data-class names as redaction facts, not full Plan Result
  `Redaction` wire objects.

Approved evidence wording:

Additional partial evidence that public call-authority validation reports typed
data-class denial facts, including redactions, for capability-forbidden classes,
call-local forbidden intersections, and reflector LM target egress, including a
non-reflector-subject bypass case.
