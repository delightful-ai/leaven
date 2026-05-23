# Score Output Current Blocker Review

Reviewer: McClintock (`019e5718-98aa-7db3-9599-cc6078f35401`)

Scope: `ps1.evaluator.score_output`

Decision: blocked; row remains pending.

Findings:

- Public-seam `submit_assessments` validation can still pass a dummy `Score.output` when the same dummy is echoed through `evidence.public.summary`. That proves internal echo consistency, not relation to the assessed candidate or artifact.
- Runtime typed outputs still allow arbitrary explicit `candidate.artifact` declarations as the runner authority. Runtime checks reject explicit `candidate.output` dummy declarations and many scorer-side forgeries, but they still do not independently prove relation to the artifact.
- The Plan IR `submit_assessments` shape can carry candidate and candidates fields, but the current semantic validator does not independently bind `Score.output` to those candidate identities.

Current status:

- Keep `ps1.evaluator.score_output` pending.
- Existing runtime and public-seam evidence remains useful partial evidence: missing output, blank output, cross-context output, same-context dummy, mutable-context forgery, explicit candidate-output dummy declarations, and several public-only dummy cases are rejected.
- Do not mark the row proven until public-seam validation has an independently checkable candidate/artifact output binding, and runtime no longer treats arbitrary explicit `candidate.artifact` declarations as sufficient relation proof.
