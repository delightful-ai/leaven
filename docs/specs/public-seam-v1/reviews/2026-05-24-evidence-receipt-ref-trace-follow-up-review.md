# Evidence ReceiptRef And Trace Receipt Follow-Up Review

Date: 2026-05-24
Reviewer: Archimedes (`019e5924-a977-7d72-82e1-636539018b15`)
Scope: partial evidence for `ps1.evidence.visibility_receipts`.

## Reviewed Change

- Evidence envelopes accept the locked schema's string-form and object-form
  `ReceiptRef` values in `source_receipts`.
- Public and top-level evidence trace refs expose normalized `trace_receipts`
  when the optional `TraceRef.receipt` field is present.
- Present trace receipts must be declared in `source_receipts`.
- Plan Result evidence validation uses the same normalized receipt refs and
  still rejects wrong receipt kinds and undeclared present trace receipts.

## Initial Finding

Important: the first implementation required every evidence `TraceRef` to carry
`receipt`, but `common.schema.json` makes `TraceRef.receipt` optional. That was
stronger than the locked schema and therefore spec drift.

Minor: wrong-kind evidence receipt rejection was only proven with string-form
refs. The implementation normalized object-form refs, but the proof was thin.

## Resolution

`collect_trace_facts` now treats `TraceRef.receipt` as optional. It collects and
normalizes a trace receipt only when present, and present trace receipts must
still be declared in `source_receipts`.

`evidence_envelope_accepts_schema_valid_trace_refs_without_receipts` proves
schema-valid unreceipted trace refs remain accepted while trace data classes
still propagate.

`plan_result_rejects_evidence_source_receipts_that_are_missing_or_wrong_kind`
now includes an object-form wrong-kind negative: `source_receipts.effect`
contains `receipt_ref("qrec_target")` while `lmrec_score` remains declared, so
trace declaration stays satisfied and the rejection targets the effect receipt
kind mismatch.

## Sign-Off

Critical: none.
Important: none remaining.
Minor: none blocking.

The reviewer signed off this tranche as partial evidence that standalone
evidence envelopes and nested Plan Result evidence accept object-form
`ReceiptRef`s, preserve optional trace refs without receipts, normalize present
trace receipts, reject undeclared present trace receipts, and reject wrong-kind
evidence source receipts through Plan Result validation.

`ps1.evidence.visibility_receipts` remains pending. This does not prove stale
receipt fingerprint semantics, runtime/provider evidence production, or
end-to-end evidence emission outside the public-seam validator path.

## Verification Evidence

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test evidence_envelope -- --nocapture`
- `cargo test -p leaven-public-seam --test plan_result_evidence -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --exact --nocapture`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
