# Evaluator Target Read Follow-Up Review

Reviewer: Einstein (`019e5705-3acb-7531-9740-7247783940e6`)

Scope: `ps1.evaluator.target_reads`

Decision: may be promoted.

Findings:

- No blocking findings.
- The prior Debug leak is closed: `ScoreCase` uses a custom `Debug` implementation that does not format target material, and there is no public `ScoreCase::target()` accessor.
- Target access routes through `ScoreContext::load_target()` / `JudgeScoreContext::load_target()` and records `CaseDataReadEvidence` with `operation = case_query.load`, field `target`, and data class `case.target`.
- The public-seam representative Plan IR path still requires capability-authorized execution, checks evaluation run/request scope, maps `target` to `case.target`, and emits query receipts.

Recorded limits:

- Proven scope is the public `leaven-run` scorer/judge target access path, assessment case-data read evidence, and public-seam representative `case_query.load` receipt/capability checks.
- Not proven by this row: ACP transport delivery, capability minting policy, durable receipt persistence, or a full runtime evaluator service beyond these public-seam and run-layer paths.
