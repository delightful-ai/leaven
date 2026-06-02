## Boundary

`leaven-seam-service` owns configured executable service implementations behind
the public seam runtime. It may compose the locked `leaven-public-seam` Plan IR
executor, provider-neutral effect traits, and concrete local/mock provider crates
that are explicitly configured for a serve process.

It must not own stdio framing, CLI argument parsing, graph internals, schema
validation policy, or provider protocol details. Transport stays in
`leaven-seam-stdio`, dispatch and response validation stay in
`leaven-seam-runtime`, and concrete provider adapters stay in their provider
crates.

## Verification

When changing executable service behavior, run:

```bash
cargo test -p leaven-seam-service
```

If dependencies or crate boundaries change, also run:

```bash
cargo test -p leaven --test topology_contract
```
