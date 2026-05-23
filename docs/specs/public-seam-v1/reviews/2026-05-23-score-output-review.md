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

Limits:

- This review does not sign off `ps1.evaluator.score_output`.
- No matrix status should change for this row until pairwise/listwise behavior, unrelated-output rejection, null-placeholder rejection, public-seam `OutputRecord` integration, executable positive/negative tests, and a follow-up adversarial sign-off exist.
