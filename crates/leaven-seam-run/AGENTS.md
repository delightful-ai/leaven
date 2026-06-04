## Boundary

`leaven-seam-run` owns the run-bound public SDK route composition layer above
`leaven-run` and `leaven-seam-service`.

It may load the locked public-seam package, construct the transport-neutral
runtime, bind a run-owned `leaven-seam-service` adapter while a real run/stage is
active, expose that route through stdio adapters, and provide CLI-facing route
helpers.

It must not own optimizer strategy, graph mutation, run graph internals, public
wire schemas, line-framing mechanics, provider protocols, Python SDK ergonomics,
or configured service-mode provider execution. Graph mutation remains in
`leaven-engine` through `RunContext`; run-builder durability remains in
`leaven-run`; method semantics/adapters remain in `leaven-seam-service`; dispatch
and response validation remain in `leaven-seam-runtime`; line-delimited transport
remains in `leaven-seam-stdio`.

## Route Here

- Product SDK-run orchestration that must know both ordinary durable run
  lifecycle and public-seam worker serving.
- Route helpers for launchable `leaven seam ...` commands that bind a live run to
  external-language worker callbacks.
- Deterministic no-spend tests proving run-bound service composition, durable
  checkpoint persistence, and Rust checkpoint/graph readback through this route.

## Route Away

- Standalone configured method execution belongs in `leaven-seam-service` plus
  `leaven seam serve --stdio`.
- Bidirectional bridge-demo/provenance belongs in `leaven-acp-stage-bridge` and
  the legacy top-level `leaven serve --stdio --plan --out` route until removed.
- Public-seam schemas/profile/capability validation belong in
  `leaven-public-seam`.
- Python `lv.optimize(...).run()` wrappers belong in `sdk/python`; they may spawn
  this route but must not become the source of Rust graph truth.

## Verification

When changing this crate, run:

```bash
cargo test -p leaven-seam-run
```

If dependencies, workspace membership, or CLI route wiring changes, also run:

```bash
cargo test -p leaven --test topology_contract
```
