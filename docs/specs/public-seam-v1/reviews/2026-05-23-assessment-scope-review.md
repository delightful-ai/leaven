# Public Seam V1 Assessment Scope Review

Scope: `ps1.evaluator.assessment_scope` after adding public-seam and engine evidence links.

Reviewer:

- Codex adversarial review in active goal continuation.

Verdict:

- `ps1.evaluator.assessment_scope`: partial evidence only. The row must remain pending.

Evidence that now exists:

- Engine `RunContext::evaluate_with(...)` records assessments under the evaluation request id, evaluator id, candidate/case target shape, evidence reference, and graph-visible assessment ids.
- Engine pairwise and listwise tests prove assessments are not written into a single global id bucket; request shape and candidate target shape are preserved in graph queries.
- Public-seam Plan Result validation requires `submit_assessments` write receipts to be backed by a matching assessment-batch value with the same evaluation request id and assessment ids.
- Public-seam Plan IR validation rejects `submit_assessments` entries missing `score` or per-assessment `replayability`.

Why this is not checkoff:

- The row's positive proof requires valid assessments to attach to the expected evaluation request, candidates, cases, scores, evidence, and replayability statuses across the intended public seam path.
- Current evidence is split between engine runtime records and public-seam document validation. There is still no producer that lowers the runtime assessment records into a public Plan Result envelope through the public owner.
- No adversarial sub-agent sign-off has approved this row as proven.

Allowed updates:

- Keep the row pending with these evidence links.
- Build the runtime-to-public assessment batch projection and receipt path before requesting sign-off.

Not allowed:

- Do not mark `ps1.evaluator.assessment_scope` proven from engine-only graph records or public-seam schema validation alone.
- Do not claim global-bucket fake passes are fully rejected until the runtime-to-public path is executable.
