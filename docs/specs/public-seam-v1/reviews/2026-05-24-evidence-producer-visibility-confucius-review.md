# Evidence Producer Visibility Confucius Review

Date: 2026-05-24
Reviewer: Confucius (`019e5b78-caa8-7a10-9e45-8c984942838d`)

Scope:

- `ps1.evidence.visibility_receipts`
- Public run/evaluator assessment producer projection into public-seam Plan
  Results.

Reviewed tranche:

- `crates/leaven-run/src/public_seam/assessment_write.rs`
- `crates/leaven-run/tests/public_seam.rs`
- `crates/leaven-public-seam/src/result.rs`
- `crates/leaven-public-seam/tests/plan_result_evidence.rs`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`

Review method:

- Read-only adversarial semantic review against the locked public-seam specs,
  schemas, conformance matrix fake-pass traps, current code, tests, and prior
  blocker reviews.
- The reviewer was explicitly instructed that rerunning the same tests was not
  sign-off.

Findings:

- Critical: none.
- Important: none blocking row promotion.
- Important: the tranche closes the prior Popper blocker for
  `ps1.evidence.visibility_receipts`. The producer path now goes through
  `RunContext` and `ScoringEvaluator`, captures `ScoreContext::load_target()` as
  `CaseDataReadEvidence`, persists it in `CaseAssessmentEvidence`, projects it
  through `submit_assessments_plan_result_with_evidence`, and validates the
  resulting Plan Result through `leaven-public-seam`.
- Minor: the proof covers the run/evaluator assessment producer path. It is not
  a universal claim that every future producer of `CaseAssessmentEvidence` is
  impossible to misuse.

Resolution:

- Promote `ps1.evidence.visibility_receipts` to `proven`.
- The sign-off is limited to the row's stated visibility-in-values-and-receipts
  requirement and its fake-pass trap.
- This sign-off does not close broader rows for full data-class propagation,
  evaluator score-output identity, ACP delivery, workspace lifecycle, or
  unrelated producer families.

Verification evidence from main rollout:

- `cargo fmt --check`
- `cargo test -p leaven-run --test public_seam -- --nocapture`
- `cargo clippy -p leaven-run --test public_seam -- -D warnings`
- `cargo test -p leaven-public-seam --test plan_result_evidence -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package -- --nocapture`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
