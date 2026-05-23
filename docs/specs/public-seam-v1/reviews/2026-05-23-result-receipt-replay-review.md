# Public Seam V1 Result, Receipt, Replay Review

Scope: pending result/receipt/replay rows after public-seam PlanResult validation work.

Rows reviewed:

- `ps1.result.typed_envelope`
- `ps1.receipts.audit_currency`
- `ps1.receipts.failed_costs`
- `ps1.replay.per_assessment`
- `ps1.visibility.data_class_propagation`

Fresh evidence before review:

- `docs/specs/public-seam-v1/conformance-matrix.yaml`
- `docs/specs/public-seam-v1/03_result_receipts_spec_v0.3.md`
- `docs/specs/public-seam-v1/schemas/leaven.plan_result.v1.schema.json`
- `crates/leaven-public-seam/src/result.rs`
- `crates/leaven-public-seam/src/package.rs`
- `crates/leaven-public-seam/tests/plan_result.rs`
- `crates/leaven-public-seam/tests/plan_result_replayability.rs`
- `crates/leaven-public-seam/AGENTS.md`

Adversarial reviewer:

- Sub-agent `019e54f0-3a03-7862-87a0-3b00bc8ac13b`

Review result:

- No reviewed row is signed off. All reviewed rows must remain pending.

Blocking findings:

- `ps1.result.typed_envelope`: blocked. Current evidence validates fixed PlanResult JSON fixtures through the public seam, but the matrix row requires success and failure plan runs to produce typed PlanResult values. The crate maturity text also limits `PlanResultDocument` to wire-envelope validation, not runtime production.
- `ps1.receipts.audit_currency`: blocked. Current semantic checks link values to receipt existence and operation kind, but they do not recompute or validate mismatched `op_hash`, `request_hash`, or `result_hash`, and no replay path proves receipts are audit currency rather than accepted decorations.
- `ps1.receipts.failed_costs`: blocked. Current proof uses a fixed failed-call fixture, not a controlled failing LM, agent, or sandbox call that incurs cost through the engine budget ledger. Validation checks charge receipt presence and back-reference only.
- `ps1.replay.per_assessment`: blocked. The validator rolls up supplied per-assessment replayability, but no evaluator/run path produces `assessment_batch_receipt` values with per-assessment replayability.
- `ps1.visibility.data_class_propagation`: blocked. Current validation proves nested data-class coverage for some result values, but not monotonic propagation through projections, templates, LM calls, agent runs, writes, receipts, and redaction reporting.

Supporting evidence that exists:

- `crates/leaven-public-seam/tests/plan_result.rs` rejects generic result blobs, missing capability or policy fingerprints, untyped errors, unknown error codes, missing receipt timing, missing receipt refs, wrong receipt kind refs, and failed paid calls without matching charge receipts.
- `crates/leaven-public-seam/tests/plan_result_replayability.rs` proves supplied mixed per-assessment replayability rolls up correctly, rejects a plan-level summary hiding non-replayable assessments, rejects missing `per_assessment`, and requires submit-assessment write receipts to match an assessment batch scope.
- `crates/leaven-public-seam/src/result.rs` validates receipt timing, value-to-receipt kind references, failed paid call charge back-references, assessment batch scope, replayability roll-up, and nested result data-class coverage.

Limits:

- This review does not sign off any reviewed row.
- Public-seam validation evidence is useful prerequisite evidence, but it is not runtime/public-owner production proof.
- No matrix status should change for these rows until producer/runtime evidence, executable positive and negative tests matching each row, and a follow-up adversarial sign-off exist.

Audit-currency fixture follow-up:

- `crates/leaven-public-seam/tests/plan_result.rs::plan_result_accepts_query_call_and_write_receipts_as_audit_currency` now validates one Plan Result carrying query, call, and write receipts referenced by typed result values.
- `crates/leaven-public-seam/tests/plan_result.rs::plan_result_rejects_decorative_or_wrong_kind_receipt_refs` also rejects duplicate operation receipt ids, so receipt ids cannot act as ambiguous log-correlation decoration.

Current limits after fixture follow-up:

- This still does not sign off `ps1.receipts.audit_currency`.
- The row remains pending because the public-seam validator still proves supplied receipt shape/roles, not a runtime replay path that recomputes every operation hash from producer-owned preimages.

Audit-currency value-binding follow-up:

- `crates/leaven-public-seam/src/result.rs::validate_result_hash_bindings` now recomputes JCS/SHA-256 result hashes for typed values bound to query, call, and write receipts and rejects same-prefix hashes that do not match the value payload.
- `crates/leaven-public-seam/tests/plan_result.rs::plan_result_rejects_same_prefix_result_hashes_that_do_not_bind_values` covers mismatched query, call, and write `result_hash` values that keep the `fp_result_sha256_` audit role prefix.
- This is useful evidence for `ps1.receipts.audit_currency`, but the row remains pending until a follow-up adversarial review signs off the complete audit-currency proof, including whether request/op hashes are sufficiently producer-bound.

Audit-currency Plan-preimage follow-up:

- `crates/leaven-public-seam/src/package.rs::PublicSeamPackage::validate_plan_execution_result` now validates an externally supplied Plan Result against the active Plan document and execution context, then `crates/leaven-public-seam/src/plan_execution.rs::validate_plan_result_receipts` recomputes representative query `op_hash`, call/write `request_hash`, write `result_hash`, read-scope fingerprints, and projection fingerprints from Plan IR preimages and result bindings.
- `crates/leaven-public-seam/tests/plan_document.rs::plan_execution_result_rejects_receipt_hashes_unbound_from_plan_preimages` rejects same-prefix query op-hash, call request-hash, write request-hash, write result-hash, and tampered-plan preimage mismatches.
- This is useful evidence for `ps1.receipts.audit_currency`, but the row remains pending until a follow-up adversarial review signs off the complete receipts tranche.

Receipts-tranche adversarial review:

- Sub-agent `019e5632-b3d5-7fe0-9f58-185752793859` reviewed tranche revset `yzrpzvol::owqmrpko` and blocked `ps1.receipts.audit_currency` and `ps1.receipts.failed_costs`.
- Blocking findings: `validate_plan_result_receipts` did not require receipts for every preimage-verified Plan IR operation; `ps1.receipts.failed_costs` still lacks engine budget-ledger/runtime evidence; `submit_assessments` result hashes were skipped; replay-mode receipts are supplied artifacts rather than Plan-preimage-bound audit currency.
- Follow-up resolution: `validate_plan_result_receipts` now requires query/call/write receipts on non-replay execution paths, rejects replay mode as preimage proof, rejects extra or missing `op_var` coverage, and `execute_plan_document` invokes the verifier for non-replay modes. `plan_execution_result_rejects_receipt_hashes_unbound_from_plan_preimages` now also covers missing write receipts, missing query receipts, and vanished failed paid call receipts/charges.
- Follow-up resolution: `submit_assessments` is no longer skipped by `validate_result_hash_bindings`; `plan_result_rejects_submit_assessment_result_hashes_that_do_not_bind_values` rejects same-prefix `submit_assessments` result-hash mismatches.
- Current status: `ps1.receipts.audit_currency` still remains pending until follow-up adversarial sign-off. `ps1.receipts.failed_costs` remains pending because engine budget-ledger/runtime evidence is still absent.

Request-evaluation follow-up review:

- Sub-agent `019e5632-b3d5-7fe0-9f58-185752793859` re-reviewed `ps1.receipts.audit_currency` at `vkpwzpmn` and still blocked sign-off because generic Plan Result validation skipped `request_evaluation` result hashes while the matrix row claimed write receipts generally.
- Follow-up resolution: `PublicSeamPackage::validate_plan_result_document` now rejects `request_evaluation` receipt/value pairs without an evaluation job context; `PublicSeamPackage::validate_evaluation_request_receipt_document` uses the context-aware Plan Result path and then verifies the evaluation-job request/result hashes through `EvaluationRequestReceiptDocument::from_plan_result`.
- `evaluation_request_receipt_binds_job_candidate_and_case_identity` proves the dedicated route accepts the context-bound request-evaluation receipt, and `evaluation_request_receipt_rejects_decorative_or_unbound_hashes` rejects decorative request and result hashes.
- Current status: `ps1.receipts.audit_currency` still remains pending until this request-evaluation fix receives adversarial follow-up sign-off.

Request-evaluation extra-receipt follow-up review:

- Sub-agent `019e5632-b3d5-7fe0-9f58-185752793859` re-reviewed `ps1.receipts.audit_currency` at `ykwwkvwy` and still blocked sign-off because the context-aware request-evaluation path validated only the matching job receipt while allowing extra decorative `request_evaluation` receipts/values.
- Follow-up resolution: generic Plan Result validation now rejects any context-free `request_evaluation` receipt, including unreferenced receipts. The evaluation-job route now rejects unexpected extra `evaluation_request_receipt` values and unexpected extra `request_evaluation` write receipts before validating the single job-bound receipt.
- `evaluation_request_receipt_rejects_decorative_or_unbound_hashes` now also rejects an extra decorative `request_evaluation` value/receipt pair with same-prefix hashes.
- Current status: `ps1.receipts.audit_currency` still remains pending until this extra-receipt fix receives adversarial follow-up sign-off.

Submit-assessments request-hash follow-up review:

- Sub-agent `019e5632-b3d5-7fe0-9f58-185752793859` re-reviewed `ps1.receipts.audit_currency` at `rqlzuyom` and still blocked sign-off because generic Plan Result validation bound `submit_assessments` result hashes and scope but only prefix-checked the `request_hash`.
- Follow-up resolution: `validate_submit_assessments_request_hash` now recomputes the `submit_assessments` request hash from the evaluation request id and assessment ids carried by the write receipt.
- `plan_result_rejects_submit_assessment_result_hashes_that_do_not_bind_values` now rejects both same-prefix `submit_assessments` request-hash scope mismatches and result-hash value mismatches.
- Current status: `ps1.receipts.audit_currency` still remains pending until this submit-assessments request-hash fix receives adversarial follow-up sign-off.

Nested score-output data-class follow-up:

- `crates/leaven-public-seam/tests/plan_result_evidence.rs::plan_result_rejects_nested_score_output_data_class_gaps` now proves a result value containing assessment rows must include nested `score.output.data_classes` such as `candidate.output` in the value-level `data_classes`.
- This is useful evidence for `ps1.visibility.data_class_propagation`, but it still does not sign off that row: monotonic data-class propagation through projections, templates, LM calls, agent runs, writes, receipts, and redaction reporting remains pending.
- The same fixture is also useful prerequisite evidence for `ps1.evidence.visibility_receipts`, paired with `evidence_envelope_preserves_visibility_data_classes_and_receipts` and the source-receipt kind negatives. That row still remains pending because this validation layer does not prove runtime evidence production or persisted receipt visibility from the evaluator path.

Failed-cost follow-up:

- `crates/leaven-public-seam/tests/plan_document.rs::plan_execution_produces_failed_paid_lm_call_and_charge_receipts` now proves the public-seam execution harness can produce a failed paid `lm_complete` call receipt, a linked charge receipt, and a typed `PlanError`.
- `crates/leaven-public-seam/tests/plan_result.rs::plan_result_rejects_failed_call_costs_without_charge_receipts` now also rejects linked charge receipts whose cost is smaller than the failed call cost.
- This is useful evidence for `ps1.receipts.failed_costs`, but the row remains pending until the follow-up receives its own adversarial sign-off.
