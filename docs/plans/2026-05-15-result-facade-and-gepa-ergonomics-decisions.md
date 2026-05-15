# Decisions for the morning — result facade & GEPA ergonomics

- **Date:** 2026-05-15
- **Status:** decision request — needs Darin's call, deliberately not done autonomously
- **Why deferred:** both are public-API *design* choices on core types, driven
  by a spec marked `planning` (not an implemented contract). The whole
  cleanup conversation established that rushing public-API shape produces the
  next bad abstraction. These are flagged, not slop-skipped: the analysis below
  is complete enough to decide from.

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

### Recommendation

**B now, A as a deliberate follow-up.** `best: Option<CandidateId>` is a
correctness fix and should land regardless — the current type can represent
"a best candidate" for a run that has none. `events` typing should ride with
the facade decision (don't type it twice). The full `Optimized<P, S>` reshape
(A) is worth doing but is a real design session — the `summary: S` generic and
`GepaSummary` shape deserve the same scrutiny the reflection types got. Don't
let a planning sketch become code by autopilot.

**One thing to settle first:** is the `summary: S` generic actually carrying
its weight, or is a plain non-generic `Optimized` with an optional
`gepa: Option<GepaSummary>` field simpler and honest? That is the core taste
call and belongs to you.

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

### Recommendation

Build the two constructors `Gepa::reflect_with_lm(lm, model)` and
`Gepa::reflect_with_agent(workspace, runtime)` as thin convenience entry points
that return a `Gepa` builder pre-wired with the default reflector — **without**
the full fluent `.prompt_template/.render/.materialize` type-state chain.
`prompt_template` is already reachable via `LmBackedReflectorConfig`; the
selection seam via `.reflective_dataset(...)`. The constructors are the cheap,
high-value, low-risk 80%; the fluent sub-knob chain is the speculative 20% and
can wait for real demand. This is a small slice once you bless the two
constructor signatures.
