## Boundary

`leaven-seam-service` owns configured executable service implementations behind
the public seam runtime. It may compose the locked `leaven-public-seam` Plan IR
executor, provider-neutral effect traits, configured local subprocess stage
workers, and concrete local/mock provider crates that are explicitly configured
for a serve process.

It must not own stdio framing, CLI argument parsing, graph internals, schema
validation policy, or provider protocol details. Transport stays in
`leaven-seam-stdio`, dispatch and response validation stay in
`leaven-seam-runtime`, and concrete provider adapters stay in their provider
crates. Subprocess stage workers are a configured service implementation here,
and worker-initiated callback requests are serviced through the configured
service while the stage is active. Their public wire remains the locked
`leaven/stage.run` and `leaven/*` JSON-RPC methods.

Current executable method status is product-facing and is recorded in
`../../docs/specs/public-seam-v1/executable-method-status.md`. Update that file
in the same change when this crate adds or removes configured service behavior,
changes a method from mock-only to live-provider-backed, or deliberately marks a
method unsupported.

## Map

- `service.rs`: configured Plan IR service composition for LM, workspace, and
  agent effects.
- `graph_state.rs`: private serve-process graph readback state for configured
  Plan IR graph writes. It may record schema-valid public graph rows for
  read-after-write proof inside one `leaven seam serve --stdio` process; it is
  not Rust `RunGraph` or durable checkpoint storage.
- `lm.rs`: configured LM provider selection for mock and OpenAI-backed
  `leaven/lm.complete`. Public dependencies are `leaven-lm` and configured
  provider crates; provider protocol details stay in the provider crates.
- `stage.rs`: runner/proposer stage service configuration, dispatch, and
  callback loop. Public dependencies are `leaven-public-seam` stage/effect
  semantics and the standard library subprocess boundary; private helpers stay
  in this module.

## Verification

When changing executable service behavior, run:

```bash
cargo test -p leaven-seam-service
```

If dependencies or crate boundaries change, also run:

```bash
cargo test -p leaven --test topology_contract
```
