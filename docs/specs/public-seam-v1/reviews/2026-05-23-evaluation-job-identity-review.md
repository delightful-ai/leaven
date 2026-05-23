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

Adversarial review:

- Reviewer: sub-agent `019e5525-e070-7be3-adee-781067a24e84`
- Result: blocked. The row must remain pending.

Blocking findings:

- The public-seam evidence validates caller-supplied `EvaluationJob` JSON but does not prove that runtime evaluation lowering creates schema-valid public jobs or evaluation request receipts.
- `EvaluationJobDocument` preserved only counts and a resolved-set id, not the candidate ids or case ids needed to prove candidate/case-set binding.
- Pairwise self-pair denial compared raw JSON values, so alternate `CandidateRef` spellings for the same candidate could bypass the denial.
- Explicit case sets were accepted without `case_set_version` and `partition_summary`, and cursor-only sets could pass as resolved.
- Missing-identity negatives did not cover `evaluation_request_id`, `base_revision`, or `evaluator_id`.

Implementation after block:

- `EvaluationJobDocument` now preserves normalized case ids and candidate ids in addition to counts.
- Pairwise self-pair validation now compares normalized candidate ids across string and object `CandidateRef` spellings.
- Nonempty resolved sets now require explicit `case_ids`, `case_set_version`, and `partition_summary`; cursor-only sets no longer pass as partition-resolved.
- `crates/leaven-public-seam/tests/evaluation_job.rs::evaluation_job_rejects_missing_identity_deadline_or_capability` now covers missing request id, base revision, and evaluator id.
- `crates/leaven-public-seam/tests/evaluation_job.rs::evaluation_job_rejects_unresolved_case_sets_and_invalid_pairs` now covers mixed-ref self-pairs, cursor-only fake resolution, and missing partition-resolution fields.

Fresh verification after adversarial block follow-up:

- `cargo fmt --check`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-public-seam --test evaluation_job`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --exact`
- `CARGO_INCREMENTAL=0 cargo clippy -p leaven-public-seam --tests -- -D warnings`

Current limits after adversarial block follow-up:

- This still is not an adversarial sign-off and does not mark `ps1.evaluator.job_identity` proven.
- The row remains pending because runtime evaluation lowering still does not produce an evaluation request receipt through the public seam owner.

Runtime projection follow-up:

- `leaven-run` now exports advanced public `PublicEvaluationJobContext` lowering from an engine `EvaluationRequestView` into a schema-valid `leaven.evaluation_job.v1` document for validation by `leaven-public-seam`.
- `crates/leaven-run/tests/scoring_evaluator.rs::runtime_evaluation_requests_project_to_public_seam_evaluation_jobs` proves real `RunContext::evaluate_with(...)` independent, pairwise, and listwise requests project request id, candidate ids, resolved case ids, base revision, deadline, evaluator id/fingerprint, and capability fingerprint through the locked public-seam validator.
- The same test rejects `AssessmentGranularity::Both`, which has no locked V1 evaluation-job wire representation.

Current limits after runtime projection follow-up:

- This still is not an adversarial sign-off and does not mark `ps1.evaluator.job_identity` proven.
- The row remains pending because the positive proof also requires an evaluation request receipt bound to the candidate and case sets; this slice projects the job document but does not yet emit or validate that receipt.
