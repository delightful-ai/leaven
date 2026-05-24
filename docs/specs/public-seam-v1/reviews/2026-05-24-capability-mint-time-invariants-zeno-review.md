# 2026-05-24 Capability Mint-Time Invariants Review

Reviewer: Zeno (`019e5b37-f0f2-7b91-a479-37ab9924b523`)

Scope:

- `crates/leaven-public-seam/src/capability.rs`
- `crates/leaven-public-seam/tests/capability_document.rs`
- `crates/leaven-public-seam/tests/call_authority.rs`
- Matrix rows: `ps1.capability.document_truth`,
  `ps1.capability.grant_enforcement`

Prompt notes:

- Passing the parent-run tests was explicitly not sufficient sign-off.
- Review focus was spec drift, fake passes, missing negative tests, topology
  leaks, public-maturity overclaiming, and whether proven capability rows were
  made suspect by the tranche.

Initial finding and resolution:

- The first mint-time invariant test bundled all target-bearing grant forms in
  the same runner/reflector fixture, so an implementation that rejected only
  one target path could pass. Resolved by splitting reflector negatives across
  independent schema-valid cases for `case_fields: ["target"]`,
  `allowed_input_classes: ["case.target"]`, and non-`none` target egress.

Follow-up verdict:

- No blocking findings for recording partial mint-time subject/grant semantic
  evidence.
- The implementation stays in the public-seam capability document owner and
  does not introduce runtime, provider, or engine leakage.
- Existing proven capability rows are not made suspect by this tranche.

Residual limitations:

- This proves `CapabilityDocument::from_value` rejects schema-valid
  subject/grant mismatches before authorization or execution.
- It is not runtime evaluator closeout and must not broaden
  `ps1.evaluator.assessment_scope`.
- Runner target-bearing denial remains a representative composite case while
  reflector coverage is split across the three target-bearing grant dimensions.

Approved evidence wording:

Additional partial evidence that capability document validation enforces
locked mint-time subject/grant invariants that JSON Schema cannot encode:
runner/reflector stage-call subjects cannot receive target-bearing grants, and
`evaluation_stage_call` assessment-submit grants must stay within the subject
evaluation request.
