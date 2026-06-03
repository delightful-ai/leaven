## Boundary

`leaven._seam_optimize` owns the current private `lv.optimize(...).run()`
mechanics path over the durable public seam server.

It may lower the public Python composition into configured
`leaven seam serve --stdio` calls and project those results into the public
`Optimized` facade. It must not own optimizer strategy, graph mutation,
capability policy, provider adapters, or Python worker process dispatch.

## Public Dependencies

- Public SDK composition records: artifacts, cases, environments, runtimes,
  and `Optimized` result types.
- The durable `leaven seam serve --stdio` CLI route and locked public-seam
  runner `leaven/stage.run` request/result schemas.

## Private Dependencies

- `leaven._seam` config, request, and one-shot process client helpers.
- No imports from `leaven._serve`; that module is the legacy bridge-demo path
  and must not be part of current `lv.optimize(...).run()` mechanics.

## Map

- `types.py`: private report/assessment records for the mechanics path.
- `scoring.py`: deterministic local score projection for current prompt slice.
- `driver.py`: durable seam process requests for runner-stage mechanics.
- `__init__.py`: map-only re-export.
