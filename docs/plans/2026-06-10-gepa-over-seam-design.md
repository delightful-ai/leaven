# GEPA Over The Seam: Real Optimization Behind `lv.optimize`

**Date:** 2026-06-10
**Status:** approved design, pre-implementation
**Decided with:** darin (interactive session)

## Problem

`lv.optimize(seed, environment, optimizer, runtime).run()` is the product, but
today it runs `run_prompt_mechanics`: evaluate the seed, call the proposer
once, return `best_score = seed_score`. The real `leaven-gepa` loop (frontier,
reflection, admission gate) has never been bound to the durable seam route.
Every moving demo (`leaven serve` legacy bridge, the live AgentKit e2e) uses a
hand-rolled mini-optimizer.

## Goals

1. **AIME:** `lv.optimize` runs real GEPA on AIME prompts through the durable
   seam: gpt-4.1-mini solver (OpenAI direct, Rust-owned `leaven-lm-openai`),
   gpt-5.4-mini LM-backed reflection, at shrunk p8 scale (~10 train cases,
   ~30 metric calls; paper scale stays a flag on the same code path).
2. **Codex-kit:** `lv.optimize` runs real GEPA on an AgentKit (system prompt +
   skill files) evaluated on one terminal-bench-2 task (`regex-log`, n=1) via
   harbor, with live Codex (gpt-5.4-mini) as the agentic
   reflection/proposal path.

**Cutoff per goal:** the first optimizer iteration where GEPA authors a
*changed* artifact, applies it through `RunContext`, and the child is
**re-evaluated onto the frontier**. Apply-without-re-eval does not count.

**First-class requirements:** agentic materialization/reflection (Codex sees
real rollout traces and edits the kit in a materialized workspace), and full
instrumentation (GEPA events, effect receipts, costs, rollout traces — durable
and inspectable from Python).

## Non-goals (this effort)

- litellm / Python-owned LM execution (LM stays Rust-owned for receipts/budget;
  a Python LM provider worker is a separate future slice).
- LM judges (`judge=` stays empty; both rubrics are programmatic).
- Watch/streaming wire semantics (deferred from V1; see instrumentation).
- Full benchmark numbers (cutoff is one real iteration; scale is a flag).
- Making harbor a `leaven-workspace` backend (harbor is rollout-internal infra).

## Architecture

Rust owns the loop. Python authors the problem and serves rollout/rubric
stages. The seam carries dispatch, receipts, and durable state.

```
Python client                 Rust `leaven seam serve --stdio`          Python worker subprocess
─────────────                 ────────────────────────────────          ────────────────────────
lv.optimize(...).run()
  ── leaven/optimize.run ──►  seam-service: lower request into
                              leaven-run builder + Gepa optimizer
                              engine loop: Gepa::step() ─ per metric call:
                                ── leaven/stage.run (runner) ──────────►  @lv.runner (target-free,
                                ◄─ output + effect receipts ───────────   cx.lm → leaven/lm.complete callback)
                                ── leaven/stage.run (rubric) ──────────►  @lv.reward rubric (ScoringCaseView)
                                ◄─ reward vector ──────────────────────
                              assessments → RunContext::submit_assessments
                              reflection: Rust-native (LM-backed or
                              AgenticProposer/Codex) → proposal →
                              RunContext::apply_batch → child re-eval
  ◄─ Optimized projection ──  durable run/checkpoint + event log
lv.runs.open / inspect ─────► Rust-owned readback (proven path)
```

### Host: `optimize_run_service` module in `leaven-seam-service`

**Amended 2026-06-10 after grounding (originally a new `leaven-gepa-seam`
crate).** The host needs no new crate and no new traits: `leaven-run`'s
builder already IS the loop (`optimize(seed).train(...).runner(closure)
.score(closure).using(gepa).budget(...).run()` drives
`Engine::run_iterations` → `Gepa::step`, with checkpoints per iteration —
exactly how p8 runs). The seam host is therefore *configured composition*,
which is `leaven-seam-service`'s stated mandate, in the same crate that
already owns the private `CommandRunner` worker-dispatch machinery and the
`SeamTextProblem` precedent for in-crate problem types:

- Worker-dispatching closures satisfying leaven-run's existing `Runner` /
  `Scorer` closure seams: runner dispatches a `runner` stage over
  `leaven/stage.run` to the configured worker argv; scorer dispatches a
  `scorer` stage and lowers the typed `StageScoreFact` reward vector into the
  engine `Score`. Worker callbacks (`leaven/lm.complete`, capability-gated
  `leaven/case.target` during scorer stages only) are serviced by the host.
- Prompt problem binding in-module (`SeamPromptArtifact` + single-part
  `EditSurface`), following the `SeamTextProblem` precedent. Promotion to a
  `leaven-artifact-prompt` crate is deliberate future work once a second
  consumer exists; the legacy `leaven-acp-stage-bridge::PromptArtifact` is
  not a dependency target for new public behavior.
- `handle_optimize_run` lowers the validated request into the leaven-run
  builder with a `Gepa::reflect_with_lm(...)` reflector and projects
  `RunResult` + frontier into `OptimizeRunResultDocument`.
- New dependency edge: `leaven-seam-service` → `leaven-gepa` (composition,
  like configured LM providers). GEPA search policy stays in `leaven-gepa`;
  wire law stays in `leaven-public-seam`; graph mutation stays behind
  `RunContext`.

### Wire contract revisions (deliberate locked-V1 changes)

Three additions, each with schema + codegen + worker-profile + method-status +
contract-test updates in the same change:

1. **`StageRunKind::Rubric`.** Scoring becomes a dispatched worker stage.
   Preserves structural target isolation: runner stages stay target-free
   (`InputCaseView`); rubric stages receive scoring-scoped case access
   (`ScoringCaseView`). Result carries the reward **vector**
   (id/value/weight/feedback per reward), not a pre-collapsed scalar.
2. **`leaven/optimize.run`.** The product method. Params: seed artifact record,
   case manifest, optimizer config (population size, budget, minibatch,
   objective — `instance` maps to `ParetoFrontier`; `hybrid`/`cartesian` stay
   validate-only), reflection config (LM-backed model name or agentic/codex),
   runtime config (LM providers, budget ceiling), worker argv for stage
   dispatch. Result: typed `Optimized` projection (best, frontier with
   parent/lineage and scores, iterations, applied proposal receipts, cost
   totals) plus the durable run/checkpoint reference for `lv.runs` readback.
3. **Agent-kit artifact wire record.** Named parts: system prompt + skill
   files (`{path, content}` list) — a *projection* of the Git-backed AgentKit
   (`GitProgramArtifact`; corrected 2026-06-11: there is no flat-content
   AgentKitArtifact). The host constructs a real run-scoped Git repo from wire
   content and reads child revisions back to flat parts. This opens the
   declared non-`PromptArtifact` boundary for one real artifact type.

Hard cutover: `run_prompt_mechanics` and the Python-side scoring path are
deleted when `optimize.run` lands. No parallel old/new paths.

### Reflection / proposal paths

- **AIME:** Rust-native `Gepa::reflect_with_lm(...)` (`LmBackedReflector`,
  `DefaultReflectionRenderer` + `PlainTextEditParser`) with gpt-5.4-mini via
  `leaven-lm-openai`, mirroring p8. Python is not in the reflect path.
- **Codex-kit:** `AgenticProposer` + `CodexAgentKitMaterializer` +
  `leaven-agent-codex-cli` (gpt-5.4-mini, `LEAVEN_CODEX_LIVE=1`,
  `LEAVEN_CODEX_BIN`). The `ReflectRequest` examples must carry real rollout
  evidence: per-case instruction, harbor verifier output, reward, and the
  Codex trajectory/transcript (as evidence refs + excerpts projected into the
  reflective workspace). Codex edits the parent kit in the materialized
  workspace; readback produces the typed proposal; `RunContext::apply_batch`
  admits the child; the child re-rolls through harbor.

### Python SDK changes (`sdk/python`)

- `optimize.py` / `_seam_optimize`: replace the mechanics driver with the
  `optimize.run` client call; lower `Environment`/`Rubric`/`gepa(...)`/
  `Runtime` into the request; surface `Optimized` from the durable result.
- `_seam_worker` / `_stage_runtime`: serve the new rubric stage role (dispatch
  registered `@lv.reward` rewards over `ScoringCaseView`), alongside runner.
- Harbor integration (codex-kit example package): pin `harbor==0.13.1`;
  `LeavenCodex(Codex)` agent subclass (via `import_path`) that uploads the
  materialized AgentKit (`AGENTS.md` + skills) into the container WORKDIR
  before `codex exec`; rollout = one `Trial` per case returning
  `{reward, ctrf passed/total, tokens, cost_usd, trajectory ref}`.
  Harbor stays inside the rollout function; its spend is reported back as
  rollout evidence (not capability-gated like `cx.lm`) — accepted V1 caveat,
  documented at the owning surface.
- AIME cases: reuse `examples/p8_aime_gepa/scripts/materialize_hf_aime.py`
  output (`AI-MO/aimo-validation-aime` train/val; `MathArena/aime_2025` held
  out) loaded through `lv.cases`.

### Instrumentation (first-class)

- Every `GepaEventSummary` (iteration started, reflection started/completed,
  candidate admitted/rejected, optimization ended) flows through the existing
  event sink into `RunContext::emit` → durable run events, readable post-run
  via `lv.runs.inspect` / `leaven/graph.query` (both proven). Server mirrors
  events to stderr as structured lines for live visibility (V1-compliant; wire
  streaming stays deferred).
- Effect receipts from every stage dispatch (LM tokens, cost, agent calls)
  aggregate into the budget ledger and the final `Optimized` cost totals,
  p8-style.
- Harbor trial artifacts (trajectory.json, ctrf.json, transcript) are captured
  as evidence attached to assessments — the same records reflection consumes.
- The Python result exposes: frontier with lineage and scores, per-case reward
  vectors, applied proposal receipts, cost/token totals, and the run ref for
  full event readback.

## Verification policy

Mock-first, live-last, per slice:

- Wire revisions: `cargo test -p leaven-public-seam --test public_seam_contract`,
  Python codegen/codec tests, `cargo test -p leaven --test topology_contract`.
- Host crate: deterministic tests with mock LM scripts and a scripted worker —
  including the loop law: *changed child + applied + re-evaluated onto the
  frontier* with `MockRunner`-class determinism.
- SDK: `cd sdk/python && just check`; example tour stays no-spend by default.
- Live proofs (recorded with exact commands + artifacts, never in default
  tests): AIME cutoff run (`LEAVEN_LIVE_OPENAI=1`, gpt-4.1-mini solver,
  gpt-5.4-mini reflection, ~10 cases / ~30 metric calls); codex-kit cutoff run
  (`LEAVEN_CODEX_LIVE=1`, harbor `regex-log`, Docker local).
- `just check` at integration milestones; `just release-check` at closeout.

## Slices (ordered)

1. **Wire:** `StageRunKind::Rubric`, `leaven/optimize.run`, agent-kit artifact
   record — schemas, codegen, profile, method-status, contract tests.
2. **Host:** `leaven-gepa-seam` crate (worker-backed runner/rubric bridges,
   prompt problem binding, event/receipt plumbing, checkpointing) +
   `leaven-seam-service` `optimize.run` handler. Deterministic loop proof.
3. **SDK cutover:** Python `optimize.run` client + rubric-serving worker;
   delete the mechanics driver; example 03 becomes a *real* mock-LM
   optimization again (seed improves).
4. **AIME e2e:** AIME case loading + example; mock proof, then live cutoff run.
5. **Codex-kit e2e:** agent-kit wire record in SDK, harbor rollout package,
   agentic proposer slot; deterministic kit-evolution proof, then live cutoff
   run on `regex-log`.

Each slice is a coherent jj commit set; AGENTS.md / method-status docs update
in the same change as the boundaries they describe.

## Key risks

- **Locked-V1 churn:** three deliberate contract revisions; mitigated by doing
  them first, in one slice, with the full schema/codegen/profile proof chain.
- **Engine seam fit:** `WorkerRunner`/`WorkerRubric` must satisfy leaven-run's
  existing evaluation seams without leaking seam types into the engine; if the
  seams don't fit, fix the seam at its owning layer rather than wrapping.
- **Harbor version churn:** pin `harbor==0.13.1` and the task git commit;
  `LeavenCodex` subclasses their adapter, so refresh the vendored clone first.
- **Subprocess-per-stage-dispatch cost:** tolerable at cutoff scale (AIME ≈30
  metric calls; harbor dominates codex-kit anyway); persistent workers are a
  later optimization, not v1.
