# GEPA/AIME Parity Goalcraft

This is the ready-to-paste `/goal` draft for the implementation pass. It points
at the detailed spec and handoff instead of restating the whole design.

```text
/goal Destination: Implement GEPA AIME paper-parity readiness in /Users/darin/src/personal/leaven against docs/specs/gepa_aime_paper_parity.md, docs/specs/durable_runs_and_resume.md, and docs/plans/2026-05-15-gepa-aime-parity/goal-handoff.yaml. End state: a real durable/resumable high-level `leaven::optimize(...).using(Gepa...).budget(Budget::metric_calls(500)).run()` path for P8/AIME with GEPA-shaped budget stopping, loop semantics, reflection, LM/provider/cache, AIME data/runner/scorer/reporting.

Context: durable checkpoint/resume substrate is assumed available from the current stack; do not replace this with examples or internal-only proof.

Scope: implement engine budget/stoppers, GEPA sampler/parent/part/acceptance/validation/cache/continuation behavior, reflection renderer/parser/LM/cost path, and P8 AIME materializer/runner/scorer/report according to the spec. Keep crate boundaries: engine owns run/stop/cache execution, leaven-run owns public builder/report lowering, leaven-gepa owns GEPA strategy state, LM crates own provider/cache, P8 owns benchmark adapter.

Preserve: no Python shell-out/provider bypass; no one-iteration scaffold as default GEPA; no JSON reflection unless configured; no validation/test oracle leakage into default reflection; no `0.0` fallback for absent scores; no graph internals in ordinary user API; no claiming CAIS numbers until live run proves them.

Verify: add focused engine/GEPA/reflection/AIME tests named or equivalent to spec section 9; run `just milestone-p8`; run bounded live OpenAI P8 slice through Leaven LM path if credentials exist; run `just check` or report any unrelated existing SLA blocker with evidence.

Done/stop: done only when handoff acceptance items are proven or explicitly blocked with evidence; final closeout must compare implementation to handoff and list remaining GEPA CAIS deltas. Stop for missing credentials only after deterministic/provider-mock proof is complete.
```
