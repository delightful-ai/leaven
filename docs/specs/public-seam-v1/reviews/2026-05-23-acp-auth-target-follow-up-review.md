# ACP Auth And Target Reads Follow-Up Review

Reviewer: Einstein (`019e5705-3acb-7531-9740-7247783940e6`)

Scope: `ps1.acp.auth_permissions`, `ps1.evaluator.target_reads`

Decision:

- `ps1.acp.auth_permissions`: may be promoted.
- `ps1.evaluator.target_reads`: still pending at review time.

Findings:

- No blocker remained for `ps1.acp.auth_permissions`. The reviewer found that authenticate now requires an expected fingerprint, permission authorization requires an `AcpAuthenticatedSession`, mismatched session/capability fingerprints deny, missing grant dimensions are projected, and denied model/workspace/sandbox/case-target paths assert PlanError-shaped decisions.
- P1 blocker for `ps1.evaluator.target_reads`: `ScoreCase<I, T>` derived `Debug` while carrying private `target: Option<T>`. A scorer with `T: Debug` could inspect target material through `format!("{:?}", ctx.case)` without calling `ScoreContext::load_target()` and without recording `CaseDataReadEvidence`.

Recorded limits for `ps1.acp.auth_permissions`:

- Proven scope is public-seam wire-contract/session primitives and programmatic grant denials.
- Not proven by this row: ACP process runtime, transport loop, renewal lifecycle, durable bearer-token handling, or runtime effect execution.

Resolution status:

- `ps1.acp.auth_permissions` can move to `proven` with the limits above.
- `ps1.evaluator.target_reads` remains pending until the Debug target leak is fixed and follow-up review signs it off.
