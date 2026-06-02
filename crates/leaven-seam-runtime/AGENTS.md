## Boundary

This crate owns the transport-neutral public-seam request dispatcher. It loads
the locked public-seam package/profile, validates incoming Leaven JSON-RPC
requests, routes every locked `leaven/*` method to a `SeamService`, and validates
successful service responses before a transport writes them.

It is not a stdio/HTTP adapter, process supervisor, optimizer loop, provider
runtime, graph mutation owner, or schema-codegen crate. It must not know how
bytes arrive; transport adapters such as `leaven-seam-stdio` own that.

## Route Here

- Classifying a JSON-RPC request as `leaven/stage.run` versus Plan IR effect
  method.
- Validating the request envelope and params through `leaven-public-seam`.
- Defining the `SeamService` trait that runtime owners implement.
- Wrapping service success into validated JSON-RPC success responses.
- Returning JSON-RPC error responses for malformed requests or unavailable
  service methods.

## Route Away

- Line reading, flushing, EOF behavior, and inherited stdin/stdout belong in
  `leaven-seam-stdio`.
- Worker process spawning and the legacy bidirectional transport machinery stay
  in `leaven-acp` until a future hard rename/migration.
- Provider execution stays in `leaven-lm*`, `leaven-agent*`,
  `leaven-workspace-*`, and engine/run owners.

## Proof Anchors

- `tests/runtime_contract.rs` proves every locked method in the active worker
  profile validates and reaches a transport-neutral `SeamService`; malformed or
  unknown methods return JSON-RPC errors instead of fake success payloads.

## Verification

- Run `cargo test -p leaven-seam-runtime` after changing dispatch behavior.
- Run `cargo test -p leaven --test topology_contract` after changing crate
  dependencies or workspace routing.
