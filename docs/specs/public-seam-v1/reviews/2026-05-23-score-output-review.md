# Public Seam V1 Evaluator Score Output Review

Scope: `ps1.evaluator.score_output` across `leaven-run` evaluator evidence and the public-seam `OutputRecord` contract.

Fresh evidence before review:

- `crates/leaven-run/src/evidence.rs`
- `crates/leaven-run/src/evaluator.rs`
- `crates/leaven-evidence/src/feedback.rs`
- `crates/leaven-evidence/src/command.rs`
- `crates/leaven-run/tests/scoring_evaluator.rs`
- `crates/leaven-run/tests/optimize_builder.rs`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`
- `docs/specs/public-seam-v1/schemas/common.schema.json`

Adversarial reviewer:

- Agent id: `019e5497-1189-72b2-a835-36fcda15ec47`

Review result:

- `ps1.evaluator.score_output`: blocked. The row must remain pending.

Blocking findings:

- Pairwise and listwise assessments are not proven. The row requires independent, pairwise, and listwise assessments, but `ScoringEvaluator::evaluate` currently accepts only `ResolvedRequestKind::Independent`; existing tests assert pairwise rejection.
- Unrelated or dummy output is not rejected. `Score::with_output` accepts any scorer-supplied `OutputRecord`, and `evaluate_job` only checks that some output exists.
- Null-placeholder output is not covered by a row negative. Missing `Score.output` is covered, but not a public/runtime negative for placeholder output that exists only to satisfy schema shape.
- The public-seam `OutputRecord` shape is not integrated with the run/evidence `OutputRecord`. The schema requires `kind`, `visibility`, and `data_classes`; the current `leaven_evidence::OutputRecord` carries `Inline` or `BlobRef` only.

Non-blocking evidence that exists:

- Independent scoring preserves a typed runner output through scoring into `CaseAssessmentEvidence.output()`.
- Missing `Score.output` fails with charged cost.
- The run builder path accepts score-supplied report output for typed runner output.

Follow-up implementation after block:

- `leaven-run` now requires `Score::with_output(...)` to receive a `ReportableOutput` minted by the active `ScoreContext`; `ScoreContext::report_output(...)` and `report_text_output(...)` are the only public construction path.
- `crates/leaven-run/tests/scoring_evaluator.rs::scoring_evaluator_rejects_report_output_from_another_scoring_context` rejects reusing output from a different candidate/case scoring context.
- `crates/leaven-run/tests/scoring_evaluator.rs::scoring_evaluator_rejects_empty_placeholder_report_output` rejects whitespace-only inline report output.
- `leaven-evidence` now owns `OutputMetadata`, `OutputVisibility`, `DataClass`, and `DataClassSet`; `crates/leaven-evidence/tests/command.rs::output_record_preserves_visibility_and_data_classes` proves annotated output records preserve these facts.
- `leaven-public-seam` now projects reusable output records into `common.schema.json#/$defs/OutputRecord`; `crates/leaven-public-seam/tests/output_record.rs` proves inline projection, blob metadata requirements, and schema rejection for missing visibility/data classes.
- `leaven-public-seam` now semantically validates `submit_assessments` Plan IR score outputs across independent, pairwise, and listwise assessment items; `crates/leaven-public-seam/tests/plan_document.rs` rejects missing schema output, blank text output, and null JSON output.
- `leaven-run` now has `JudgingEvaluator` runtime production evidence for pairwise and listwise requests; `crates/leaven-run/tests/scoring_evaluator.rs` proves pairwise/listwise report outputs are preserved and rejects missing, placeholder, and cross-context group outputs.
- This resolves the earlier implementation gaps, but the row still needs fresh full verification and adversarial sign-off before any matrix status change.

Follow-up adversarial review:

- Reviewer: sub-agent `019e54fa-c9ca-7520-9d19-a1038f1795ab`
- Result: blocked. The row must remain pending.

Follow-up blocking findings:

- Same-context nonblank dummy output is still accepted. `ReportableOutput` scope and placeholder checks reject missing, blank, and cross-context output, but a scorer can still mint arbitrary nonblank text from the correct `ScoreContext` or `JudgeScoreContext`.

Follow-up implementation after second block:

- `crates/leaven-run/tests/scoring_evaluator.rs::runtime_score_outputs_project_through_public_seam_for_all_assessment_shapes` now starts from runtime independent, pairwise, and listwise assessments and lowers their `CaseAssessmentEvidence.output()` values through `PublicSeamPackage::project_output_record(...)` into the locked `common.schema.json#/$defs/OutputRecord` wire shape.
- `RunOutput::new(...)` now declares its string value as the assessed reportable output, and typed runners must explicitly declare the assessed rendering with `RunOutput::typed(...).with_reportable_output(...)` or `.with_reportable_text(...)`.
- `ScoreContext::report_output(...)` and `JudgeScoreContext::report_output(...)` still mint context-bound `ReportableOutput` values, but evaluator lowering now rejects any reported record that does not exactly match the runner-declared assessed output.
- `crates/leaven-run/tests/scoring_evaluator.rs::scoring_evaluator_rejects_same_context_dummy_report_output` and `::judging_evaluator_rejects_same_context_dummy_report_output` prove same-context dummy output is rejected for independent and judge paths.
- `crates/leaven-run/tests/scoring_evaluator.rs::scoring_evaluator_rejects_typed_score_output_without_runner_declaration` proves typed outputs cannot be made reportable solely by scorer-side rendering.

Fresh verification after second follow-up:

- `cargo fmt --check`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-run --test scoring_evaluator`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-run --test optimize_builder run_builder_typed_output_uses_score_supplied_report_output -- --exact`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-run`
- `CARGO_INCREMENTAL=0 cargo clippy -p leaven-run --tests -- -D warnings`
- `cargo test -p leaven-public-seam --test output_record`
- `cargo test -p leaven-public-seam --test plan_document submit_assessments -- --nocapture`
- `CARGO_INCREMENTAL=0 cargo check -p p8_aime_gepa`
- `CARGO_INCREMENTAL=0 cargo test -p leaven --test gepa_parity`
- `cargo test -p leaven --test topology_contract`

Limits:

- This review does not sign off `ps1.evaluator.score_output`.
- No matrix status should change for this row until a follow-up adversarial sign-off is recorded against the current code and executable evidence.

Second follow-up adversarial review:

- Reviewer: Codex parent-thread adversarial review for active goal `019e550a-a531-7cb2-b691-e4a1c5b93b79`
- Result: blocked. The row must remain pending.

Second follow-up blocking findings:

- Independent scoring could still forge the assessed output by mutating public `ScoreContext.output` before calling `report_text_output(...)`; `ScoreContext::report_output(...)` derived the expected record from the mutable field at call time.
- Runtime-to-public projection proved schema-valid public text, but not the `candidate.output` data-class fact required for a score output that is the candidate/artifact output being assessed.

Implementation after third block:

- `ScoreContext` now freezes the runner-declared reportable output in a private `expected_output` field during context construction; `ScoreContext::report_output(...)` uses the frozen declaration rather than the public mutable `output` field.
- `ReportableOutput::into_record(...)` now returns the runner-declared `OutputRecord` after the scorer-reported payload matches it, so scorer-side default metadata cannot erase the runner's visibility and data-class facts.
- `leaven-evidence::OutputRecord::candidate_inline(...)` owns the reusable public candidate-output metadata primitive, and `RunOutput::new(...)` plus `RunOutput::typed(...).with_reportable_text(...)` now use it to declare public `candidate.output` reportable output metadata.
- `crates/leaven-run/tests/scoring_evaluator.rs::scoring_evaluator_rejects_mutated_context_dummy_report_output` proves the mutable-context forgery is rejected.
- `crates/leaven-run/tests/scoring_evaluator.rs::runtime_score_outputs_project_through_public_seam_for_all_assessment_shapes` now asserts runtime-projected independent, pairwise, and listwise score outputs carry `candidate.output` data classes through the locked public-seam `OutputRecord` projection.

Fresh verification after third follow-up:

- `cargo fmt --check`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-evidence`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-run --test scoring_evaluator`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-run`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-public-seam`
- `CARGO_INCREMENTAL=0 cargo clippy -p leaven-evidence -p leaven-run --tests -- -D warnings`
- `CARGO_INCREMENTAL=0 cargo test -p leaven --test gepa_parity`
- `CARGO_INCREMENTAL=0 cargo test -p leaven --test topology_contract`

Current limits:

- This note records the resolved blockers and local verification, but it still does not sign off `ps1.evaluator.score_output`.
- No matrix status should change for this row until a new adversarial sub-agent sign-off reviews this third follow-up.
