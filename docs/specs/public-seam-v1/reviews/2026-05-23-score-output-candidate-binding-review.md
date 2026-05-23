# Public Seam V1 Score Output Candidate Binding Review

Scope: `ps1.evaluator.score_output` after the fourth score-output follow-up and attempted row checkoff.

Reviewer:

- Codex adversarial review in active goal continuation.

Verdict:

- `ps1.evaluator.score_output`: blocked. The row must remain pending.

Blocking findings:

- The public-seam `submit_assessments` validator rejects public-only dummy outputs, but it still has no independent candidate-output value to compare against. A nonblank dummy labelled with `candidate.output` or `candidate.artifact` passes the current semantic check because `Score.output` is the only candidate-output-bearing value in the assessment document. Data-class labels alone are not proof that the output is the candidate or artifact output actually assessed.
- The runtime typed-output path rejects scorer-side dummy output, mutable-context forgery, cross-context reuse, missing runner declarations, and public-only runner declarations. It still cannot prove that an arbitrary typed runner declaration labelled as `candidate.output` is the actual typed value being assessed; for opaque typed `Out`, the runner declaration is the current authority.

Why this blocks checkoff:

- The row's negative proof requires unrelated output to be rejected.
- The row's named fake pass is adding a dummy output field solely to satisfy schema validation.
- Current evidence rejects several concrete fake passes, but it does not reject a candidate-labelled dummy in the public seam or an arbitrary candidate-labelled typed runner declaration in runtime.

Allowed updates:

- Keep the row pending.
- Keep the existing positive and negative evidence as partial evidence for the narrower claims it actually proves.
- Add a future binding primitive or row split that makes candidate-output identity independently checkable before marking this row proven.

Not allowed:

- Do not mark `ps1.evaluator.score_output` proven.
- Do not claim that `Score.output.data_classes` alone proves the output is related to the assessed candidate.
- Do not claim the dummy-output fake pass is fully rejected.

Follow-up implementation after this block:

- `crates/leaven-public-seam/tests/plan_document.rs::submit_assessments_rejects_missing_or_placeholder_score_output` now includes a candidate-labeled dummy `Score.output`; Plan IR validation requires `Score.output` to be projected by matching `evidence.public.summary`, so the candidate data-class label is no longer accepted by itself.
- `crates/leaven-run/tests/scoring_evaluator.rs::scoring_evaluator_rejects_typed_runner_candidate_labeled_dummy_declaration` proves independent typed runners cannot use an explicit `OutputRecord::candidate_inline("dummy")` declaration as the assessed output.
- `crates/leaven-run/tests/scoring_evaluator.rs::judging_evaluator_rejects_typed_runner_candidate_labeled_dummy_declaration` proves the same denial for pairwise/listwise judging.
- `leaven-run` now tags runner reportable-output declarations internally as derived or explicit. Explicit `candidate.output` records are refused by evaluator lowering; string outputs and `with_reportable_text(...)` remain derived candidate-output declarations, and explicit `candidate.artifact` records remain allowed.

Fresh verification after follow-up:

- `cargo fmt --check`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-public-seam --test plan_document`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-run --test scoring_evaluator`

Current limits after follow-up:

- This follow-up rejects the concrete candidate-labeled dummy paths named above, but it still does not sign off `ps1.evaluator.score_output`.
- The row remains pending until a fresh adversarial review decides whether the remaining typed-output declaration authority and public-seam evidence-summary projection are strong enough for the locked row wording.
