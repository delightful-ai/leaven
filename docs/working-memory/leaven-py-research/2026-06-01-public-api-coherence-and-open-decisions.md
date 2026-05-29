# Leaven Python — Public API Coherence + Open Decisions

Status: design-decision ledger, pre-spec-revision.
Date: 2026-06-01.

Subordinate to `docs/specs/leaven_python.md` (governing) and the surface
sketches / projection audit under `docs/working-memory/leaven-py-research/`.
This file exists to make the redesign's change surface and unresolved forks
concrete enough to decide. It is not yet spec.

## Verdict: the API double-carries; it isn't missing parts

The current package (`docs/specs/leaven_py/src/leaven/__init__.py`) exports
**two complete composition surfaces simultaneously**:

- **Surface 1 (kwargs-god):** `optimize` + `OptimizeBuilder` + six decorators
  (`runner`, `scorer`, `reflector`, `proposer`, `judge`, `evaluator`) +
  `register_stage` + `serve_stage`. Used by examples 01, 03, 04, 05, 06, 07, 09.
- **Surface 2 (stage-objects):** `evolve` + `EvolutionBuilder` + `Stages` +
  `Rollout` + `ScoreStage` + `Reflect` + `Propose` + `Evaluate`. Used by
  example 10.

Both are in `__all__`. That is the incoherence — not a missing piece, a
duplicated one. It directly violates `leaven_python.md:579` ("Don't
proliferate ways to do one thing… one canonical way").

**Ingredient check.** Of the nouns the verifiers-aligned shape needs, only
three are missing from the package: `Environment`, `Rubric`, `@reward`.
Everything else already exists. **The fix is subtraction + three additions.**

## Canonical shape (decided across the 2026-06-01 conversation)

```text
optimize( seed,  environment,  optimizer,  runtime )
                     │
   Environment( task,  rollout,  rubric )      ← inner loop, yoinked from verifiers
   optimizer.gepa( …, reflect=, propose=, judge= )  ← outer loop, Leaven-native
```

- **Inner loop = `Environment`** (task + rollout + rubric): the verifiers
  `Env` analog — a named, shareable bundle. Rollout defaults to the
  **Codex-native** agent substrate; the agent owns its own multi-turn loop
  (path A — no verifiers `Harness` port).
- **Outer loop = the optimizer** (reflect / propose / judge): no prior art in
  prime-rl/verifiers to copy; this is Leaven's actual contribution.
- **`Rubric` carries the reward *vector*** (named rewards + weights). A
  composite scalar is opt-in; the optimizer declares the reduction. No forced
  scalarization.
- **`Score` stays scalar+feedback+output**; per-reward metrics live on the
  `Rubric`/evidence layer above `Score` (consistent with the projection audit).
- **`output` defaults to the rollout's final agent message** (final LM text for
  non-agent rollouts); `output=` on the rollout is an optional projection
  override (parsed JSON, files, diff). **`cx` is the primary surface** —
  workspace, session, lm/agent, effects, and role-scoped case access all flow
  through it. `output` is just the convenience default. (Resolved 2026-06-01.)

## Change inventory (what to actually edit)

1. **Collapse to one composition surface.** Keep one verb + the stage-object
   composition; remove the kwargs-god path (or demote it to thin sugar for the
   ≤2-stage prompt case only).
2. **Add three nouns:** `Environment`, `Rubric`, `@reward` — wire into
   `__all__` and the owning modules.
3. **Re-home the six decorators:** `runner → Rollout.fn`, `scorer`/`reward →
   Rubric`, `reflector`/`proposer`/`judge → optimizer stages`. Decorators
   survive as *authoring sugar* (wrap a fn → stage object); the god-function
   that takes all six as kwargs dies.
4. **Rewrite the spec.** `leaven_python.md` §"What the user writes" already
   shows the stage-object shape — good. But §"The Python authoring surface"
   still teaches `optimize(…, runner=, scorer=, reflector=, …)`, and the
   acceptance gate + "What success is not" reference `lv.optimize()` directly.
   Reconcile all three to the `Environment × optimizer × runtime` + `Rubric`
   shape.
5. **Rewrite the examples.** 7 of 10 are on the surface we'd demote (01, 03,
   04, 05, 06, 07, 09). Re-cast 03/04/09 on `Environment`/`Rubric`; reconcile
   01/05/06/07; make 10 (or a merged 9+10) the canonical full-coverage example;
   redo the README tour table.
6. **Naming reconciliation** (projection-audit slice 5). One verb (`optimize`
   vs `evolve`). Note `runtime` vs `environment` now mean **different things**
   (`Runtime` = execution substrate/budget/trust; `Environment` =
   task+rollout+rubric) — which actually *resolves* the old synonym confusion
   by giving them separate jobs.
7. **Pull in the still-open projection-audit reworks the new surface forces:**
   `Case` role projection (target-free runner view; rubric/scorer target via
   `cx`), `OutputRecord` kinds + rich-text returns, `AgentInstructions.developer`
   /roles, `EvidenceEnvelope` classmethod facades, `AssessmentWrite.kind` /
   `EvaluationJob.granularity` wire-name alignment, `data_class.ARTIFACT_OUTPUT
   → CANDIDATE_ARTIFACT`, `RunContext` builder role-split (runner must not get
   proposal/assessment builders).

## Open decisions (these are the confusion sources you asked about)

### D1. {==`evaluator` vs `scorer` vs `Rubric` — three scoring-shaped things==}{>>Sharpest one. My lean: `Rubric` is the default scoring surface (weighted reward vector); `scorer` is a degenerate one-reward `Rubric`; `evaluator` survives as the escape hatch for scoring that needs batched effects / custom assessments / evidence (the locked-spec dspy-codex example is evaluator-shaped, so it can't vanish). Need the boundary between Rubric and evaluator stated precisely. Your call?<<}{id="c1" by="AI" at="2026-06-01T16:30:00.000Z"}

**Resolved 2026-06-01:** Rubric is the product scoring surface. `evaluator` is
demoted to an internal/seam concept — it stays a locked public-seam *wire*
envelope and an advanced/adapter authoring path, but is **not** a Python
product-surface role. The engine lowers `Rollout + Rubric` into the
assessment/evidence/receipt machinery the locked dspy-codex evaluator writes by
hand; that example becomes the seam shape, not a front-door example.

### D2. {==Where do `reflect` / `propose` / `judge` attach?==}{>>Real fork. Example 10 bundles all five (rollout, score, reflect, propose, evaluate) in `Stages`. My lean: SPLIT — rollout+rubric live on `Environment` (the shareable, model-agnostic "what's the task + how is it scored"); reflect+propose+judge live on the optimizer (the GEPA-coupled "how do we search"). This matches the verifiers env↔trainer boundary and kills the 5-slot `Stages` grab-bag. Agree, or keep one `Stages` bundle?<<}{id="c2" by="AI" at="2026-06-01T16:30:00.000Z"}

**Resolved 2026-06-01:** SPLIT — reflect/propose/judge attach to the optimizer;
the 5-slot `Stages` bundle is dropped. Accepted as **GEPA-coupled for V1**: the
user flagged that hard-wiring the outer-loop stages to GEPA is a known wart to
revisit (the stages should become optimizer-neutral long-term), but it ships for
V1 because the only optimizer *is* a GEPA clone. Sketches 03/04 embody this, plus
D3 (seed passed separately) and D4 (sandbox declared on `Task`).

### D3. {==Is `seed` (the mutable artifact) a separate arg or inside `Environment`?==}{>>My lean: separate (mirrors verifiers, where the model-under-optimization is separate from the env). `Environment.rollout` is artifact-*type*-aware (knows how to interpret a SkillBank vs a PromptArtifact); the seed *instance* is passed to `optimize(seed=…)`. Confirm?<<}{id="c3" by="AI" at="2026-06-01T16:30:00.000Z"}

### D4. {==Who owns the sandbox — `Task` or `Runtime`?==}{>>Example 10 puts `sandbox` on `Task`. The stage-draft says Runtime *allocates*. My lean: `Task` declares the sandbox *requirement*; `Runtime` *provides/allocates* it. Make that split crisp in the types or it will confuse.<<}{id="c4" by="AI" at="2026-06-01T16:30:00.000Z"}

### D5. {==Multi-objective contract==}{>>You flagged "leaven SHOULD work multi-obj." Decide the *contract* now even if the impl scalarizes by default: `Rubric` carries the reward vector; the optimizer declares the reduction (`gepa(objective="weighted" | "pareto" | …)`). GEPA already runs a validation Pareto frontier — but that's a different axis (over instances) than reward-dimensions. Lock the contract so it isn't designed away; defer true reward-dimension Pareto.<<}{id="c5" by="AI" at="2026-06-01T16:30:00.000Z"}

### D6. {==Adopt the `load_environment(config) -> Environment` factory packaging?==}{>>verifiers' portable-module convention is also how a CLI / remote worker would load a Leaven problem. My lean: adopt it as the packaging contract (a Leaven problem = a module exposing `def environment(config) -> lv.Environment`), but it's defer-able to V2. Worth it for V1, or call-site `lv.Environment(...)` only?<<}{id="c6" by="AI" at="2026-06-01T16:30:00.000Z"}

### D7. {==`@reward` return type — `float` or rich `RewardValue`?==}{>>You said rich text returns, obviously. So a reward fn returns `RewardValue(value: float, feedback: str|RichText, output: OutputRecord|None)`, with bare `float` as the degenerate case. Decide the `OutputRecord` kinds that ship in V1 (text / json / blob / agent_session / workspace_diff).<<}{id="c7" by="AI" at="2026-06-01T16:30:00.000Z"}

### D8. {==One verb: `optimize` or `evolve`?==}{>>The product word is "optimize anything"; the gate + authoring surface already say `optimize`. My lean: keep `optimize(seed, environment, optimizer, runtime)`, retire `evolve` (or keep as a deprecated alias for one release). But "evolve" reads nicer for the FlashEvolve framing — genuinely your call.<<}{id="c8" by="AI" at="2026-06-01T16:30:00.000Z"}

### D9. {==`lv.x.verifiers` scope once we have native nouns==}{>>Shrinks to "import + run an existing verifiers `Env` module unchanged" (the migration path), NOT a parallel native surface. Confirm it's adapter-only.<<}{id="c9" by="AI" at="2026-06-01T16:30:00.000Z"}

### D10. {==Codex-native default before live proof==}{>>The redesign leans on Codex as the default rollout/materialization substrate, but no live Codex has run the loop yet (skill-bank reflection is proven only with `FakeAgentRuntime`; the 55-check smoke rubric is designed, not built). My lean: write the surface as Codex-native (right shape) but keep examples honest with the existing `NotImplementedError`-at-the-boundary discipline; the live loop is the open gate, not a silent claim. OK?<<}{id="c10" by="AI" at="2026-06-01T16:30:00.000Z"}

## Not doing

- Not porting verifiers' `Harness` (path A: the agent runtime owns multi-turn;
  custom protocols are just a loop in the runner body).
- Not taking verifiers' `Config`/Hub machinery (Leaven owns the seam/wire).
- Not scalarizing rewards by default.
