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

## Map

- `service.rs`: configured Plan IR service composition for LM, workspace, and
  agent effects.
- `stage.rs`: runner-stage service configuration, dispatch, and callback loop.
  Public dependencies are `leaven-public-seam` stage/effect semantics and the
  standard library subprocess boundary; private helpers stay in this module.

## Verification

When changing executable service behavior, run:

```bash
cargo test -p leaven-seam-service
```

If dependencies or crate boundaries change, also run:

```bash
cargo test -p leaven --test topology_contract
```
