# Score Output RunContext Plan Projection Review

Date: 2026-05-24T04:55:32Z
Reviewer: Dalton (`019e5851-35a6-7792-902c-08c0c947c4ec`)
Scope: `ps1.evaluator.score_output` partial evidence after the
RunContext-backed `submit_assessments` Plan IR projection tranche.

## Reviewed Claim

`PublicAssessmentWriteReceiptContext::submit_assessments_plan_document` lowers
graph-backed independent assessments into locked `submit_assessments` Plan IR
by loading stored `CaseAssessmentEvidence` through the configured evidence
store. The emitted `Score.output` is therefore sourced from stored assessment
evidence rather than caller-supplied raw JSON rows.

The row remains pending. This review signs off only that the tranche is honest
partial evidence.

## Findings

Initial review found no Critical or Important findings. Dalton found one Minor
test gap: the implementation documented explicit scaffolding for unsupported
pairwise/listwise assessment shapes and blob-backed score outputs, but the
tests only covered the supported independent inline path and missing evidence
store path.

Resolution: added
`assessment_score_output_plan_projection_rejects_unsupported_shapes_and_outputs`,
which asserts `UnsupportedAssessmentShape` for a stored pairwise assessment and
`UnsupportedScoreOutput` for a stored blob-backed `OutputRecord`.

Follow-up review found no remaining Critical, Important, or Minor findings.

Second follow-up tightened the independent projection so inline stored evidence
must carry `candidate.output` or `candidate.artifact` before Plan IR is emitted.
It added a positive candidate-artifact projection and a public-only inline
negative. Dalton found no Critical, Important, or Minor findings and judged the
delta spec-aligned because it uses the existing candidate-bound `output` /
`artifact` convention already enforced by public-seam semantic validation.

## Verdict

This can be recorded as partial evidence for `ps1.evaluator.score_output`, with
the row still pending. It improves the prior blocker by rejecting the fake pass
where Plan IR is synthesized from graph ids alone, but it still does not prove
full row closeout or independently prove every stored output is the actual
candidate/artifact output assessed.

## Verification

Main-agent verification for this tranche:

- `cargo fmt --check`
- `cargo test -p leaven-run --test public_seam -- --nocapture`
- `cargo clippy -p leaven-run --test public_seam -- -D warnings`
- `cargo test -p leaven-public-seam --test contract_package -- --nocapture`
- `cargo test -p leaven --test topology_contract`
- after the minor follow-up: `cargo test -p leaven-run --test public_seam assessment_score_output -- --nocapture`
- after the second follow-up:
  `cargo test -p leaven-run --test public_seam runcontext_assessment_artifact_score_outputs_project_to_public_seam_submit_assessments_plan -- --nocapture`
