# Score Output Summary-Only Partial Review

Date: 2026-05-24
Reviewer: Dirac (`019e58d2-bcf2-7980-af0d-fd4838dcabb1`)
Scope: `ps1.evaluator.score_output` partial evidence only.

## Reviewed Change

- `submit_assessments` Plan semantic validation now rejects `Score.output`
  records that carry no candidate-bound inline `value` and no explicit
  blob/trace output projection.
- The negative test rejects independent, pairwise, listwise, and
  `candidate.artifact` summary-only dummies that carry candidate/artifact data
  classes and a matching `evidence.public.summary`.
- The conformance matrix remains pending and records this only as partial
  evidence.

## Findings And Resolution

Initial minor finding: the first negative set covered independent and pairwise
summary-only candidate-output dummies, but not listwise or
`candidate.artifact` variants.

Resolution: converted the test to table-style coverage for independent,
pairwise, listwise, and candidate-artifact summary-only dummies.

Follow-up minor finding: the conformance matrix still cited only the older
placeholder-output negative. Resolution: added
`submit_assessments_rejects_summary_only_score_output_dummies` to
`ps1.evaluator.score_output` negative evidence.

## Sign-Off

Critical: none.
Important: none.
Minor: none remaining after the ledger update.

The reviewer signed off this tranche as partial evidence only for
`ps1.evaluator.score_output`.

Semantic basis:

- The change closes the documented summary-only dummy fake pass without
  changing locked schema semantics.
- The blob/trace exception is acceptable for pending public-seam proof because
  the validator still requires candidate/artifact data classes and matching
  `evidence.public.summary`, while runtime dereference/provenance remains
  outside this seam crate.
- The behavior belongs in `leaven-public-seam` because it validates public Plan
  IR documents and refuses schema-valid semantic forgeries before execution.
- Row status remains pending; this is not full evaluator/runtime closeout.

## Verification Evidence

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test plan_document submit_assessments_rejects_summary_only_score_output_dummies -- --nocapture`
- `cargo test -p leaven-public-seam --test plan_document submit_assessments_ -- --nocapture`
- `cargo test -p leaven-run --test scoring_evaluator score_output -- --nocapture`
- `cargo test -p leaven-run --test public_seam assessment_score_output -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package -- --nocapture`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
