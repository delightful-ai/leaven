## Boundary

`leaven._seam_optimize` owns the private lowering from `lv.optimize(...).run()`
into one locked `leaven/optimize.run` request against the durable public seam
host. The host runs the real `leaven-gepa` loop and dispatches runner and scorer
stages back to the configured Python worker over `leaven/stage.run`.

It may lower the public Python composition (seed, environment, optimizer,
runtime) into the typed `OptimizeRunRequestDocument`, configure the
`SeamServiceConfig` (worker argv plus rubric reward names, runtime LM provider,
runs root), refuse GEPA knobs with no V1 optimize route, and return the typed
`OptimizeRunResultDocument`. It must not own optimizer strategy, the GEPA loop,
graph mutation, capability policy, provider adapters, worker protocol
implementation, or the result-to-`Optimized` projection (that lives in
`leaven._runs.optimize_run`).

## Public Dependencies

- Public SDK composition records: artifacts, cases, environments, runtimes, and
  the `Gepa`/`Rubric` records those compose.
- The durable `leaven seam serve --stdio` CLI route and the locked
  `leaven.optimize_run.v1` request/result wire contract.

## Private Dependencies

- `leaven._seam` config, the `leaven/optimize.run` wire records, and the one-shot
  process client (`SeamClient.optimize_run`).
- `leaven._seam_worker` command-target construction (runner stage argv plus the
  rubric reward names the worker rebuilds the scorer rubric from).
- No legacy bridge-demo dependency: `leaven._serve` has been removed and must not
  return. The old `run_prompt_mechanics` per-case `stage.run` path plus its
  receipt/score/status projections were deleted in the optimize.run cutover; do
  not reintroduce a parallel mechanics route.

## Map

- `types.py`: `PlannedOptimizeCase`, the lowering-side case record.
- `driver.py`: lower the composition into one `leaven/optimize.run` request,
  configure the worker/LM/runs-root, refuse unsupported GEPA knobs naming what V1
  supports, require a `metric_calls` budget (with an optional `usd` ceiling that
  lowers into `max_cost_usd_micro`) while refusing budget axes with no V1 route
  (`calls`, `lm_tokens`, `wall_seconds`, `concurrent_calls`), and return the
  typed result outcome.
- `__init__.py`: map-only re-export.
