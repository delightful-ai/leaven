## Boundary
This crate owns in-memory implementations of `leaven-store` capabilities:
`InlineStore` for blob/checkpoint bytes and `InlineEvidenceStore<E>` for
typed evidence payloads.

It is a fast local/test/default backend, not a durable persistence story and
not an engine graph schema.

## Local Rules
- Keep namespaces explicit. Store refs should preserve the configured store
  name so callers can distinguish default inline stores from named in-memory
  stores.
- Store bytes and evidence payloads through the neutral traits. Do not add
  engine-specific record layout, run IDs, graph checkpoint semantics, or
  product-builder defaults here.
- This crate may be used by engine and run tests as a cheap dependency, but
  those tests should still assert engine/run behavior in their owning crates.

## Verification
- `cargo test -p leaven-store-inline` proves blob/checkpoint/evidence in-memory
  behavior, wrong-namespace refusal, and evidence ref round trips.
- `cargo test -p leaven --test topology_contract` proves inline storage stays
  a backend dependency instead of absorbing graph or product-builder ownership.
