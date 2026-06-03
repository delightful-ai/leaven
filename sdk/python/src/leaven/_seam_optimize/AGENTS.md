## Boundary

`leaven._seam_optimize` owns the current private `lv.optimize(...).run()`
mechanics path over the durable public seam server.

It may lower the public Python composition into configured
`leaven seam serve --stdio` calls, configure the private Python command worker,
dispatch runner/proposer mechanics, and project those results into the public
`Optimized` facade. It must not own optimizer strategy, graph mutation,
capability policy, provider adapters, or worker protocol implementation.

## Public Dependencies

- Public SDK composition records: artifacts, cases, environments, runtimes,
  and `Optimized` result types.
- The durable `leaven seam serve --stdio` CLI route and locked public-seam
  runner/proposer `leaven/stage.run` request/result schemas.

## Private Dependencies

- `leaven._seam` config, request, and one-shot process client helpers.
- `leaven._seam_worker` command target construction only.
- No imports from `leaven._serve`; that module is the legacy bridge-demo path
  and must not be part of current `lv.optimize(...).run()` mechanics.

## Map

- `types.py`: private report/assessment records for the mechanics path.
- `status.py`: private runtime-dependency status facts projected into public
  result summaries.
- `rewards.py`: Python reward-vector execution and aggregate score projection.
- `scoring.py`: deterministic aggregate helpers for current prompt slice.
- `driver.py`: durable seam process requests for registered runner-stage
  mechanics and submit-only proposer mechanics, including agent-backed
  proposal submission when the Python runtime config supplies an agent.
- `__init__.py`: map-only re-export.
