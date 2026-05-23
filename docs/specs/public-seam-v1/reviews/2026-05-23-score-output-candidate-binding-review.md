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
