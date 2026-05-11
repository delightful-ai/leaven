## Boundary
This crate is reserved for reusable renderers and materializers: reflection
prompts, surface diffs, lineage summaries, candidate trees, graph debug views,
and artifact/history materialization.

Current exported renderer/materializer names are placeholders. They must not be
used as proof that GEPA reflection, evidence rendering, or graph materializing
is product-ready.

## Routing
- Prompt rendering for an optimizer strategy can live here only after the
  request/response contract is neutral enough to share. GEPA-specific slot
  requests still belong in `leaven-gepa`.
- Engine graph debug views may read public engine views; they must not require
  private graph mutation or bypass `RunContext`.
- Workspace materialization composes `leaven-workspace`; it does not own
  workspace lifecycle, command execution, or cleanup.

## Verification
- `cargo check -p leaven-render` proves only that placeholder names compile.
- When a renderer becomes real, add golden output tests here and the caller
  test that proves the owning subsystem consumes the rendered handle correctly.
