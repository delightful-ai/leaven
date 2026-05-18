# Durable Runs And Resume

Status: design correction / implementation spec.

This spec corrects the current mismatch between Leaven's optimizer-library
promise and the implementation shape where durable state is optional plumbing.
It supersedes any implication that checkpointing is an advanced add-on for
ordinary optimizer runs.

## 1. Product Rule

Leaven's ordinary `.run()` is durable.

Durability means:

- Leaven writes enough run state to inspect, audit, and resume the next
  optimizer decision after process exit.
- Every future-affecting state is either durable, explicitly graph-derived, or
  the run refuses before starting.
- A non-durable run is an explicit escape hatch named `ephemeral`.

The user-facing question is not "does this optimizer implement a checkpoint
trait?" The user-facing question is:

```text
Can this stored run resume faithfully?
```

## 2. Public Vocabulary

Use these names at the product surface:

| Concept | Meaning |
| --- | --- |
| `RunStore` | Durable home for run graph, inputs, evidence, cache, reports, and continuation state. |
| `StoredRun` | A persisted run record addressable by `RunId`. |
| `Resume` | Continue a stored run from a clean boundary using compatible runtime code. |
| `Continuation` | Optimizer-owned state needed to choose the next decision exactly. |
| `Ephemeral` | Explicit non-durable mode. No resume promise. |

Avoid these words in ordinary user docs:

- checkpointable optimizer;
- private optimizer state;
- checkpoint schema;
- persistence backend.

Those are implementation concepts.

## 3. Ordinary User Surface

Default durable run:

```rust
let result = leaven::optimize(seed)
    .train(train)
    .validation(validation)
    .test(test)
    .runner(solver)
    .score(score)
    .using(Gepa::for_surface(surface))
    .budget(Budget::metric_calls(500))
    .run()
    .await?;
```

If no store is configured, `.run()` uses Leaven's default local `RunStore`,
rooted at `.leaven/runs/` relative to the process working directory unless an
explicit store root is configured.

The current ordinary builder layout is:

```text
.leaven/runs/<run-id>/
  run.sqlite
  lm-cache.sqlite
  blobs/
  checkpoints/
    LATEST
    <checkpoint-id>.checkpoint
  evidence/
    <evidence-key>.json
  reports/
    summary.json
```

The current implementation may still materialize only the file/blob/checkpoint
portion of this layout. The product target is defined in
`docs/specs/default_cache_storage.md`: durable run dirs provision SQLite-backed
evaluation and LM response caches by default while JSON checkpoint blobs remain
acceptable for optimizer continuation and human-readable sidecars.

`.run_dir(path)` uses `path` as the per-run directory with the same layout. If
`path/checkpoints/LATEST` exists, the builder treats the directory as a resume
handle and restores the latest checkpoint before continuing. If it does not
exist, the builder starts a new durable run in that directory.

Explicit non-durable run:

```rust
let result = leaven::optimize(seed)
    .train(train)
    .runner(solver)
    .score(score)
    .using(optimizer)
    .budget(Budget::metric_calls(20))
    .ephemeral()
    .run()
    .await?;
```

An equivalent explicit store form is allowed:

```rust
.store(RunStore::ephemeral())
```

but `ephemeral()` should be the common spelling for tests, examples, and
throwaway local experiments.

The currently implemented low-level opt-out remains
`OptimizeStore::inline(...)` through `.store(...)`; it is explicit advanced
plumbing and must not be the omitted-store default.

## 4. Resume Surface

Resume is ordinary, but runtime code is not magically serialized.

Stored run state contains durable data and fingerprints. The caller must supply
compatible live capabilities such as runner, scorer, evaluator, provider, and
optimizer constructors.

`resume_compatibility_fingerprints.md` owns the detailed compatibility manifest:
case content/splits, runner, scorer, evaluator, optimizer, LM roles, cache, and
budget domains are compared before resume performs costful work.

Shape:

```rust
let result = leaven::resume(run_id)
    .runner(solver)
    .score(score)
    .using(Gepa::for_surface(surface))
    .run()
    .await?;
```

or:

```rust
let store = RunStore::open(".leaven")?;

let result = store
    .resume(run_id)
    .runner(solver)
    .score(score)
    .using(Gepa::for_surface(surface))
    .run()
    .await?;
```

Resume must fail before running when:

- the stored run is missing;
- graph truth cannot be restored;
- stored dataset/case/artifact data cannot be restored;
- optimizer continuation is missing for an optimizer that requires it;
- continuation schema/fingerprint does not match the supplied optimizer;
- runner/scorer/evaluator fingerprints are incompatible;
- budget policy is incompatible with the stored ledger;
- the run stopped at a non-clean boundary that cannot be resumed safely.

## 5. Stored Data Contract

Default durable `.run()` requires storeable run data.

At minimum, the default local store must preserve:

- run id and run metadata;
- seed artifact and applied changes;
- train/validation/test cases or durable case references;
- split roles and case ids;
- graph truth: candidates, proposals, lineage, attempts, assessments, and events;
- evidence and feedback attachments;
- budget ledger and cost records;
- evaluation cache index and cache payload references;
- optimizer continuation;
- final report summaries and public result metadata.

If `A`, `A::Change`, `C`, evidence, annotations, or continuation state cannot be
serialized by the default store, ordinary `.run()` must fail with a typed
pre-run refusal. The user can then:

- choose `ephemeral()`;
- use storeable wrapper/reference types;
- provide a custom `RunStore` adapter.

The default must not silently degrade to non-durable execution.

## 6. Optimizer Author Surface

Ordinary users do not implement this surface. Optimizer authors do.

Preferred public advanced trait name:

```rust
pub trait ResumableOptimizer<P>: Optimizer<P>
where
    P: OptimizationProblem,
{
    fn continuation_policy(&self) -> ContinuationPolicy;

    fn save_continuation(
        &self,
        ctx: SaveContinuation<'_, P>,
    ) -> Result<Option<ContinuationSnapshot>, ContinuationError>;

    fn restore_continuation(
        &mut self,
        snapshot: Option<&ContinuationSnapshot>,
        ctx: RestoreContinuation<'_, P>,
    ) -> Result<(), ContinuationError>;
}
```

`CheckpointableOptimizer` may remain as an internal/transition name while the
implementation is corrected, but public docs and new APIs should move toward
`ResumableOptimizer`.

### 6.1 Continuation Policy

```rust
pub enum ContinuationPolicy {
    GraphDerived,
    ExplicitSnapshot {
        schema: Fingerprint,
        format: StateFormat,
    },
    EphemeralOnly {
        reason: String,
    },
}
```

`GraphDerived` means the optimizer can reconstruct its next-decision state
deterministically from graph truth and stored run metadata. Restore must test
that reconstruction is valid enough to continue.

`ExplicitSnapshot` means the optimizer owns future-affecting state that is not
fully derivable. Durable runs must persist it at clean boundaries.

`EphemeralOnly` means the optimizer can run only in explicit ephemeral mode.
Default durable `.run()` refuses before starting.

### 6.2 GEPA Policy

GEPA is `ExplicitSnapshot`.

GEPA continuation includes every non-derivable state that can affect future
decisions, including:

- train/search partition configuration;
- completed iteration or proposal counters;
- current best and observed candidates;
- population/frontier state;
- candidate selector state;
- part selector state;
- acceptance/gate state;
- batch sampler cursor and RNG state when introduced;
- validation policy cadence state when introduced;
- merge scheduler state when introduced;
- stopper/patience counters when introduced.

GEPA must not be considered product-ready for long-running/resumable use unless
all live strategy slots that affect future decisions participate in this
continuation contract.

## 7. Engine Responsibilities

`leaven-engine` owns durable execution semantics.

The engine must:

1. Create or open the run record before costful work starts.
2. Check stoppers before each optimizer step.
3. Execute optimizer steps through `RunContext`.
4. Persist graph, budget, cache, and optimizer continuation at clean boundaries.
5. Emit durable events for start, iteration boundaries, budget charges, errors,
   stop reason, and finish.
6. Refuse durable mode if the optimizer continuation policy cannot be
   satisfied.
7. Never replay committed graph mutations during restore.
8. Never charge budget twice for restored completed work.

Clean checkpoint boundaries:

- after seed insertion;
- after optimizer initialization;
- after each completed optimizer iteration;
- after finish.

The latest optimizer-resume checkpoint stays at the clean search boundary.
Final validation/test report work is a projection over that checkpoint: it may
add in-memory graph rows used to build the returned report, but it must not
advance the durable evaluation-cache index beyond the selected latest
checkpoint. If final report evaluations become independently resumable, they
need an explicit report-resume snapshot instead of being folded into the
optimizer checkpoint.

In-flight provider calls, agent sessions, workspaces, or evaluator jobs are not
resumed as if complete. They are abandoned or rerun according to the owning
stage policy after restoring the last clean boundary.

## 8. Stop And Budget Semantics

Budget bookkeeping is not itself a stopper. Budget caps are ledger truth. Stop
policy consumes budget snapshots.

GEPA-compatible metric-call stopping should behave like upstream GEPA:

- a `max_metric_calls` budget/stopper checks observed spent work before starting
  the next iteration;
- when spent metric calls are at or above the cap, the run stops cleanly and
  returns the current best;
- already-running parallel work may finish and push observed spent work slightly
  past the cap;
- `BudgetExceeded` remains a hard guard for unexpected overspend inside a
  costful operation.

Therefore:

- `leaven-engine` must wire stoppers into the run loop.
- `leaven-run` must expose metric-call budget/stop behavior without requiring
  users to learn engine stoppers.
- `leaven-gepa` must not use a one-iteration default as ordinary loop control.
  Iteration caps are explicit safety limits, not the default GEPA stopping
  semantics.

## 9. Layering And Crate Ownership

| Crate | Owns |
| --- | --- |
| `leaven-kernel` | ids, fingerprints, budget snapshots, cost records, state format ids. |
| `leaven-store` | generic blob/evidence/checkpoint store capabilities. |
| `leaven-store-file` | default local file-backed `RunStore` implementation. |
| `leaven-engine` | run loop, durable run state, stoppers, continuation enforcement, restore mechanics. |
| `leaven-run` | ordinary `optimize(...).run()` and `resume(...)` ergonomics, default store selection, typed pre-run refusals. |
| `leaven-gepa` | GEPA strategy state and `ResumableOptimizer` continuation implementation. |
| `leaven` | curated re-exports for ordinary run/resume/store APIs. |

Do not put GEPA continuation details in `leaven-run`.
Do not put provider SDK or concrete file-store assumptions in `leaven-engine`.
Do not expose graph internals to ordinary users to compensate for missing
result/resume ergonomics.

## 10. Result Contract

`Optimized` must report:

- `run_id`;
- best candidate/artifact when present;
- stop reason;
- budget snapshot;
- report summary with train/validation/test roles;
- whether the run is resumable;
- latest stored-run/checkpoint reference when durable;
- public event summaries;
- explicit absence/error for missing scores, not `0.0` fallback.

Final test results are report-only by default and must not affect optimizer
state unless an explicit policy says otherwise.

## 11. Typed Refusals

Required error classes:

```rust
pub enum RunStartError {
    MissingBudget,
    DurableStoreUnavailable,
    DefaultStoreUnavailable,
    UnstoreableArtifact,
    UnstoreableCase,
    UnstoreableEvidence,
    UnstoreableContinuation,
    OptimizerEphemeralOnly,
    ContinuationPolicyUnsatisfied,
    InvalidSplitPolicy,
    RuntimeFingerprintMissing,
}

pub enum ResumeError {
    RunNotFound,
    GraphRestoreFailed,
    DatasetRestoreFailed,
    ContinuationMissing,
    ContinuationSchemaMismatch,
    OptimizerFingerprintMismatch,
    RuntimeFingerprintMismatch,
    BudgetPolicyMismatch,
    NonCleanBoundary,
}
```

Exact enum names may change, but these decision classes must not collapse into
string errors.

## 12. Required Tests

Engine tests:

- default durable run writes graph, budget, cache, events, and continuation;
- stopper is checked before the next optimizer step;
- budget-reached stop returns current best rather than an optimizer error;
- `BudgetExceeded` inside a context operation still refuses before mutation;
- restore never replays committed proposals/evaluations or double-charges
  budget.

GEPA tests:

- GEPA continuation snapshot includes current best, observed candidates,
  population state, selector state, gate state, and completed progress;
- restore resumes with the same next parent, part, admission decision, and best;
- missing GEPA continuation in durable resume fails before running;
- one-iteration scaffold is not the default GEPA behavior.

`leaven-run` tests:

- `.run()` uses default durable local store;
- `.ephemeral().run()` uses no durable store and reports non-resumable;
- unstoreable user data refuses durable `.run()` before costful work;
- `.resume(run_id)` requires compatible runner/scorer/optimizer fingerprints;
- result exposes stop reason, resumability, and absent score states.

P8/AIME tests:

- deterministic smoke remains explicitly non-benchmark proof;
- live GEPA AIME path uses durable run mode by default;
- a forced stop at the metric-call cap returns a resumable stored run;
- resumed P8 run does not repeat already committed evaluations.

## 13. Migration Notes

Current implementation has useful pieces:

- `RunPersistence`;
- `StoreRunPersistence`;
- `RunCheckpoint`;
- `CheckpointableOptimizer`;
- GEPA's reduced `GepaCheckpointState`;
- `OptimizeStore::durable(...)`.

The correction is not to add a second public run path. The correction is to
make ordinary `.run()` durable, make `ephemeral` explicit, and enforce the
optimizer continuation contract at the run boundary.
