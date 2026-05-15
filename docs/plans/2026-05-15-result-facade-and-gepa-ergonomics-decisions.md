# Decisions - result facade & GEPA ergonomics

- **Date:** 2026-05-15
- **Status:** decided next slices; implementation not started in this note
- **Why this exists:** the overnight cleanup intentionally stopped before two
  public API shape choices. This note turns those parked questions into a
  concrete next cut without treating the planning spec as auto-implementation.

---

## Decision 1 — the result facade

### What exists now

`crates/leaven-run/src/result.rs`:

```rust
pub struct OptimizeResult<A> {
    pub run_id: RunId,
    pub best: CandidateId,          // NOT Option
    pub best_artifact: A,
    pub seed_artifact: A,
    pub report: OptimizationReport,
}
// + redundant accessors: best() -> &A, report() -> &OptimizationReport
```

`OptimizationReport` is a flat struct: `stop_reason`, `storage`, two
`BudgetSnapshot`s, three `Cost`s, six `Option<f64>` split scores,
`evaluation: EvaluationReport`, `events: Vec<String>`, and (to be deleted in
slice 6) duplicate `dataset`/`splits` fingerprints.

### What the spec wants

`gepa_public_private_surface.md` §11 (status: **planning surface contract**) —
the facade is `Optimized<P, S = StandardRunSummary>`:

```rust
pub struct Optimized<P: OptimizationProblem, S = StandardRunSummary> {
    pub run_id: RunId,
    pub best: Option<CandidateId>,   // Option — no seed-wins-by-default
    pub stop: StopReason,
    pub budget: BudgetSnapshot,
    pub summary: S,
}
// methods: best_id(), best() -> Option<&Artifact>, report() -> &EvaluationReport,
//          gepa() -> Option<&GepaSummary>, events() -> public event summaries
```

`Optimized`, `StandardRunSummary`, `GepaSummary` **do not exist in code**.
`durable_runs_and_resume.md` §10 (a governing spec) independently requires the
result to report run id, best-when-present, stop reason, budget, split-role
report, resumability, checkpoint ref, **public event summaries**, and explicit
absence for missing scores.

### The gaps

1. **`best: CandidateId` should be `Option<CandidateId>`.** Spec invariant: a
   run with no admissible candidate yields `None`; the seed does not win by
   default. The current non-optional field is a real correctness divergence.
2. **Naming:** spec says `Optimized`; code says `OptimizeResult`.
3. **`summary: S` generic** with `StandardRunSummary` default and a
   `GepaSummary` — a genuine new shape; those types must be designed.
4. **`events: Vec<String>`** is a stringly junk drawer (15 magic strings from
   an `event_name()` matcher over `leaven_engine::RunEvent`). The spec wants
   "public event summaries." Options: `Vec<RunEvent>` (reuse the engine's typed
   enum — but `RunEvent` is `extend`-routed component-author vocab) or a
   curated public summary enum. `RunEvent`'s 15 variants already carry only
   ids/counts/costs/snapshots — structurally they *are* summaries.
5. The flat `OptimizationReport` (six `Option<f64>` score fields, three costs)
   vs the spec's `report() -> &EvaluationReport` + `summary: S` split.

### Options

- **A. Reshape toward the spec.** Rename → `Optimized`, `best` → `Option`,
  introduce `summary: S` + `StandardRunSummary` + `GepaSummary`, `events`
  becomes typed. Most spec-aligned; largest change; the planning spec is a
  sketch, so several sub-shapes (what's in `StandardRunSummary`? is `S`
  worth the generic?) still need taste.
- **B. Minimal-correctness only.** Keep `OptimizeResult`, but fix `best` →
  `Option<CandidateId>` (the real bug) and type `events`. Defer the rename and
  the `summary: S` generic. Smaller, lower-risk, leaves a name/shape gap.
- **C. Update the spec to match code.** Decide `OptimizeResult` (flat) is the
  durable truth and rewrite `gepa_public_private_surface.md` §11. Cheapest;
  abandons the `Optimized`/`summary: S` design.

### Decision

Do **A-lite as a hard cutover**, not a compatibility shim and not a one-field
patch.

The next result slice should rename the ordinary completed-run facade to
`Optimized<A, S = StandardRunSummary>` and delete `OptimizeResult` rather than
leaving both names alive. The product-builder result is generic over the owned
artifact type, not over `P: OptimizationProblem`; `RunProblem<A, C>` is internal
lowering glue and should not leak into the user's result type.

The minimum shape for this slice:

```rust
pub struct Optimized<A, S = StandardRunSummary> {
    pub run_id: RunId,
    pub best: Option<CandidateId>,
    pub best_artifact: Option<A>,
    pub seed_artifact: A,
    pub stop: OptimizationStopReason,
    pub budget: BudgetSnapshot,
    pub summary: S,
    pub events: Vec<RunEventSummary>,
}
```

`StandardRunSummary` should absorb today's flattened `OptimizationReport`
payload: storage/resumability, optimizer/final-report cost, split score
summaries, and the graph-backed `EvaluationReport`. Keep it in `leaven-run`.
`GepaSummary` stays in `leaven-gepa`; `leaven-run` must not depend on GEPA
strategy state. The `S` generic carries its weight because it is the only clean
way for optimizer-specific summaries to exist later without making the ordinary
run crate know optimizer internals.

`RunEventSummary` should be a curated public enum in `leaven-run`, not
`Vec<String>` and not `leaven_engine::RunEvent` re-exported through the ordinary
route. The engine event enum is still an `extend`/harness contract; the ordinary
facade needs stable public summary vocabulary. Start with the same event kinds
and the fields ordinary users can rely on.

The no-best path must be real. Today `OptimizeBuilder::run()` maps
`run.best == None` into `OptimizeError::Optimizer`, then never builds a result.
The hard cutover should return `Optimized { best: None, best_artifact: None,
... }` when the optimizer has no admissible candidate, skip best-only final
evaluations, still report baseline/seed final evaluations where configured,
and make `best()` return `Option<&A>`. That is the correctness bug, not merely
the field type.

Do **not** add `type OptimizeResult<A> = Optimized<A>` or duplicate accessors
for old call sites. This repo is hard cutover.

### Result slice verification

- `cargo nextest run -p leaven-run --test optimize_builder`
- `cargo test -p leaven --test public_surface_contract`
- `just milestone-p8` after adapting the P8 example to `Optimized`
- `just check` before closeout

---

## Decision 2 — GEPA ergonomic constructors (design doc D6)

### What exists now

After slice 4, configuring GEPA reflection is:

```rust
Gepa::builder()
    .surface(AimePromptSurface)
    .population(...)
    .reflector(LmBackedReflector::with_default_renderer(lm, model))
    .reflective_dataset(builder)   // optional — defaults to GepaReflectiveDataset
```

The swappable seam (`ReflectiveDatasetBuilder`) and the divergence fix are
done. What is missing is the *ergonomic ladder* from design doc D6.

### What D6 proposed

```rust
Gepa::reflect_with_lm(lm, model)        // fully-defaulted LM reflector
    .prompt_template(MY_TEMPLATE)        // cheap LM-only knob
    .reflective_dataset(fn)              // selection seam
    .render(fn)                          // LM presentation
// or
Gepa::reflect_with_agent(workspace, runtime)
    .materialize(fn)                     // agent presentation
```

The slice-4 agent skipped this, with reasoning: §12's definition-of-done did
not enumerate the constructors, and a fluent type-state builder is a sizeable
speculative API surface. I agree it should not have been autopiloted.

### The gap

The minimal user program in the design doc §4 was
`.using(Gepa::reflect_with_lm(lm, model).surface(S))`. Today it is the more
verbose `Gepa::builder()...reflector(LmBackedReflector::with_default_renderer(...))`.
Not fuckery — no lie, no proxy — but the headline *ergonomic* win is unbuilt.

### Decision

Build **only the LM constructor now**:

```rust
Gepa::reflect_with_lm(lm, model)
    .surface(surface)
    .population(population) // optional, same builder ladder as today
```

This is a thin entry point over
`LmBackedReflector::with_default_renderer(lm, model)` and the existing
`GepaBuilder` ladder. It buys the headline ergonomic path without adding a
second reflection API.

Do **not** build `reflect_with_agent(workspace, runtime)` in this slice. The
current agent-backed helper needs a workspace factory, parser, and
`AgentBackedPolicy`; pretending the constructor is just `(workspace, runtime)`
would hide real policy and output-contract choices. The honest agent ergonomic
slice is a separate design:

```rust
Gepa::reflect_with_agent(factory, runtime, parser, policy)
```

or a named config object if that signature proves too noisy. That belongs after
the result facade cut because agent reflection summary/reporting is one of the
places result shape will matter.

Do **not** build the full fluent `.prompt_template/.render/.materialize`
type-state chain yet. `LmBackedReflectorConfig`, `.reflective_dataset(...)`,
and explicit reflector construction already cover advanced users. The missing
ordinary-user affordance is one defaulted LM entry point, not another builder
language.

### GEPA ergonomics slice verification

- `cargo nextest run -p leaven-gepa --test lm_reflection --test gepa_smoke`
- update one public GEPA example to use `Gepa::reflect_with_lm`
- `cargo test -p leaven --test public_surface_contract` if any facade route
  changes
