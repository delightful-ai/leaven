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

Nested score-output data-class follow-up:

- `crates/leaven-public-seam/tests/plan_result_evidence.rs::plan_result_rejects_nested_score_output_data_class_gaps` now proves a result value containing assessment rows must include nested `score.output.data_classes` such as `candidate.output` in the value-level `data_classes`.
- This is useful evidence for `ps1.visibility.data_class_propagation`, but it still does not sign off that row: monotonic data-class propagation through projections, templates, LM calls, agent runs, writes, receipts, and redaction reporting remains pending.
- The same fixture is also useful prerequisite evidence for `ps1.evidence.visibility_receipts`, paired with `evidence_envelope_preserves_visibility_data_classes_and_receipts` and the source-receipt kind negatives. That row still remains pending because this validation layer does not prove runtime evidence production or persisted receipt visibility from the evaluator path.
