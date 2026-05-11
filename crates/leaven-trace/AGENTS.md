## Boundary
This crate is the future trace-based optimizer home: execution subgraphs,
OptoPrime-style search, subgraph-as-code rendering, and trace-node vocabulary.

Current public names are scaffolding. They do not prove trace capture,
subgraph replay, or trace-driven optimization.

## Local Bait
- Durable run events and graph records belong in `leaven-engine`; this crate
  may consume public views but must not become an alternate run graph.
- Prompt/render helpers shared with other optimizers belong in
  `leaven-render` only after their contract is not trace-specific.

## Verification
- `cargo check -p leaven-trace` proves only scaffold exports.
- Real trace work needs replay/selection tests here plus engine tests when new
  public graph views or events are required.
