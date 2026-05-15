# GEPA AIME Parity Requirements Summary

Status: pre-goal synthesis.

This write-up captures the implementation stack needed after durable
checkpoint/resume work. The companion implementation spec is
`docs/specs/gepa_aime_paper_parity.md`.

## Intent

Leaven should be able to run a real GEPA AIME paper-parity experiment through
the high-level optimizer library surface:

```rust
leaven::optimize(seed_prompt)
    .train(train)
    .validation(validation)
    .test(test)
    .runner(aime_solver)
    .score(aime_score)
    .using(Gepa::...)
    .budget(Budget::metric_calls(500))
    .run()
    .await
```

The acceptance path is not "the example prints numbers." The acceptance path is
that the run is durable, resumable, GEPA-shaped, budget-stopped, backed by
Leaven LM/provider/cache infrastructure, and reports honest train/validation/test
results with case-level evidence.

## Required Stack

### 1. Budget and stopping

`Budget::metric_calls(500)` must behave like GEPA's `max_metric_calls=500`:

- stop cleanly before scheduling the next optimizer step when observed metric
  calls are at or above the cap;
- return the current best candidate and stop reason;
- preserve `BudgetExceeded` as a hard guard for unexpected in-stage overspend;
- separate optimization search budget from final validation/test report work.

### 2. GEPA loop parity

The GEPA loop must be a real iterative optimizer, not a one-iteration scaffold:

- train minibatch sampling;
- parent selection over population/frontier;
- selected part/surface mutation;
- reflection over real case feedback and traces;
- sampled-case acceptance;
- population/frontier updates;
- validation policy and held-out test semantics;
- evaluation cache and best-output tracking;
- parallel evaluation/proposal scheduling where upstream uses it;
- continuation state for every future-affecting slot.

### 3. Reflection parity

The reflector must use Leaven LM and the upstream-style GEPA reflection contract:

- default prompt renderer matches upstream GEPA's fenced replacement shape;
- rendered prompt includes current prompt/part, selected feedback, traces,
  expected solution feedback, and failure examples;
- default parser consumes fenced text, not JSON;
- reflection is async, metered, cached where configured, and records cost and
  request metadata;
- reflection model is configurable, with `gpt-5.4-mini` acceptable as the
  Leaven default for this reproduction path.

### 4. AIME task parity

The AIME example must be a real benchmark harness:

- materialize `AI-MO/aimo-validation-aime` as train/validation and
  `MathArena/aime_2025` as held-out test;
- preserve source ids and split roles;
- seed prompt starts at the upstream GEPA AIME prompt;
- solver model is `gpt-4.1-mini`, temperature `1.0`, max output tokens `32000`;
- scorer parses exact integer answers and emits GEPA/DSPy-style feedback with
  reference solution text;
- runner returns parsed answer plus raw transcript/trace, so the optimized
  artifact remains the prompt while execution evidence stays inspectable;
- report baseline/optimized train, validation, final test, metric calls, stop
  reason, run id, and resume reference.

## Proxy Proofs To Reject

- deterministic P8 smoke improvement;
- one-iteration GEPA scaffold;
- a durable run that cannot resume GEPA continuation;
- a live solver path that bypasses Leaven LM/cache;
- final numbers without case-level evidence and split semantics;
- a run stopped by `BudgetExceeded` error instead of clean budget stop.

## Completion Shape

The implementation is complete when an operator can:

1. materialize/cache AIME data;
2. run live GEPA AIME through the high-level Leaven surface;
3. stop by metric-call budget;
4. resume a stored run without repeating committed evaluations;
5. inspect baseline/optimized train, validation, and held-out test results with
   case-level feedback and traces;
6. see any remaining deltas from the GEPA CAIS artifact documented as explicit,
   bounded differences rather than hidden substitutions.

