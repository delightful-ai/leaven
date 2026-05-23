# Public Seam V1 Evaluation Job Identity Review

Scope: `ps1.evaluator.job_identity` across the public-seam EvaluationJob document owner.

Implementation note:

- `leaven-public-seam` now owns `EvaluationJobDocument` and `EvaluationJobKind`.
- `PublicSeamPackage::validate_evaluation_job_document(...)` validates the active `leaven.evaluation_job.v1.schema.json` document and then applies semantic identity checks.
- The semantic checks require request id, evaluator id, evaluator fingerprint, capability fingerprint, base revision, deadline, resolved set id, case-count consistency, and request shape counts.
- Pairwise jobs reject self-pairs. Explicit case sets reject `case_count` mismatches, and nonempty sets without case ids or a cursor are refused.

Executable evidence:

- `crates/leaven-public-seam/tests/evaluation_job.rs::evaluation_job_preserves_identity_for_all_request_shapes`
- `crates/leaven-public-seam/tests/evaluation_job.rs::evaluation_job_rejects_missing_identity_deadline_or_capability`
- `crates/leaven-public-seam/tests/evaluation_job.rs::evaluation_job_rejects_unresolved_case_sets_and_invalid_pairs`

Current limits:

- This is not an adversarial sign-off and does not mark `ps1.evaluator.job_identity` proven.
- The row remains pending because this slice proves the public document seam only; it does not yet prove that requesting an evaluation through runtime lowering creates these schema-valid jobs or emits evaluation request receipts bound to the candidate and case sets.

Follow-up runtime evidence:

- `leaven-engine` now records the evaluator fingerprint on each durable `EvaluationRequestRecord` and exposes it through `EvaluationRequestView::evaluator_fingerprint(...)`.
- `crates/leaven-engine/tests/context_services.rs::evaluation_requests_record_evaluator_fingerprint_as_runtime_job_identity` proves two runtime evaluator requests with the same evaluator id but different evaluator fingerprints leave distinct request records with their resolved case-set identity intact.

Current limits after runtime follow-up:

- This still is not an adversarial sign-off and does not mark `ps1.evaluator.job_identity` proven.
- The row remains pending because runtime evaluation records still do not carry the full public-seam EvaluationJob document: base revision, deadline, and capability fingerprint are not yet produced by the engine/run lowering path, and no evaluation request receipt is emitted through the public seam owner.
