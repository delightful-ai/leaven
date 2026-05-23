# Public Seam V1 Typed Envelope Follow-Up Review

Scope:

- `ps1.result.typed_envelope`

Reviewer:

- Read-only adversarial sub-agent Curie, id `019e56f3-959f-7743-890e-d00376f57ee1`.

Review rule:

- The reviewer was instructed not to edit files, create commits, or treat rerunning tests as proof. The follow-up used semantic inspection of the locked spec, matrix row, implementation, tests, and prior review note.

Verdict:

- `ps1.result.typed_envelope`: proven, with limits.

Sign-off evidence:

- Success plan run: `crates/leaven-public-seam/tests/plan_document.rs::plan_ir_family_lowers_and_executes_let_call_write_through_public_seam_owner` executes through `PublicSeamPackage::execute_plan_document`, validates the produced Plan Result, and proves typed `lm_response`, call/write receipts, request hash, write kind, and final revision.
- Failure plan run: `crates/leaven-public-seam/tests/plan_document.rs::plan_execution_produces_failed_paid_lm_call_and_charge_receipts` produces a failed call Plan Result with typed call receipt, charge receipt, and typed error.
- Validator: `PlanResultDocument::from_schema_valid_value` classifies typed values, receipts, errors, charges, replayability, data classes, and receipt timing.
- Negatives reject generic JSON blobs, missing capability/policy fingerprints, untyped error strings, unknown error kinds, and missing receipt timing.

Limits:

- This proves representative public-seam Plan Result production and envelope validation, not full runtime/provider/ACP delivery, complete Plan IR coverage, durable receipt persistence, or evaluator runtime production.
- Other rows still own deeper receipt audit-currency, failed-cost, data visibility, and evaluator behavior claims.
