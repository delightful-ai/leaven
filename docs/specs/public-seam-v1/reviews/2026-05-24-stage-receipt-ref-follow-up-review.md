# Stage Payload ReceiptRef Follow-Up Review

Date: 2026-05-24
Reviewer: Lagrange (`019e592c-a418-75a0-8273-2007211a4473`)
Scope: partial evidence for `ps1.stage.payload_receipts`.

## Reviewed Change

- `ReflectionResult.read_receipts` now normalizes string-form and object-form
  `ReceiptRef` values.
- `StagePayloadDocument` exposes normalized read receipt ids.
- Reflection results reject non-read receipt families such as `lmrec_*` and
  `agentrec_*` in the `read_receipts` slot.
- The object-form receipt proof flows through nested `ProposeRequest`
  consumption, not only standalone `ReflectionResult` validation.
- The conformance harness now parses partial evidence fields and validates
  test-symbol references for pending rows as well as proven rows.

## Initial Finding

Important: the first matrix update left one stale reference to
`stage_payloads_preserve_object_form_info_refs` under
`ps1.stage.payload_receipts`. The matrix reference check did not catch it
because pending-row partial test evidence was not parsed or symbol-checked.

## Resolution

All matrix references now use
`stage_payloads_preserve_object_form_info_and_receipt_refs`.

`ConformanceRow` now parses `partial_contract_test_evidence` and
`partial_contract_implementation_evidence`. `validate_matrix_references` checks
test symbols across positive, negative, and partial test evidence for every
row, and path-checks partial implementation evidence.

`conformance_matrix_reference_check_rejects_stale_pending_test_symbols`
injects the exact stale pending-row symbol from the finding and proves the
matrix reference check rejects it.

## Sign-Off

Critical: none.
Important: none remaining.
Minor: none.

The reviewer signed off this tranche as partial evidence that
`ReflectionResult.read_receipts` normalizes string/object `ReceiptRef`s, nested
`ProposeRequest` consumes the object-form reflection result, and effect receipt
families are rejected in the read-receipt slot.

`ps1.stage.payload_receipts` remains pending. This does not prove full stage
lowering, runtime stage execution, or end-to-end stage receipt production.

## Verification Evidence

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test stage_payloads -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package conformance_matrix_reference_check_rejects_stale_pending_test_symbols -- --exact --nocapture`
- `cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --exact --nocapture`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
