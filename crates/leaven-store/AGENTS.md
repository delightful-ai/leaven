## Boundary
`leaven-store` is the storage capability crate. It owns neutral traits and
store errors for persistence, not a persistence format.

Storage may know `leaven-kernel` refs and the cold `Evidence` marker from
`leaven-core`. It must not know `RunGraph`, stages, workspaces, provider
sessions, concrete backends, or optimizer checkpoint schemas.

## Map
- `src/blob.rs`: `BlobStore` and `BlobWrite` for opaque bytes addressed by
  `BlobRef`.
- `src/evidence.rs`: typed `EvidenceStore<E>` for problem-owned evidence
  values addressed by `EvidenceRef`.
- `src/checkpoint.rs`: byte-oriented `CheckpointStore` and `CheckpointBytes`.
  Schema ownership stays with the caller that writes the bytes.
- `src/error.rs`: `StoreError` for unavailable stores, missing refs,
  serialization failures, and typed operation failures.
- Backend crates implement these contracts: `leaven-store-inline` for in-memory
  tests/dry runs, `leaven-store-file` for local filesystem layout, and future
  object/sqlite backends for their own concrete layouts.

## Local Helper Stack
- Use `BlobStore` for operational bytes such as prompt renders, raw provider
  responses, and command output too large for metadata.
- Use `EvidenceStore<E>` for typed problem evidence that the graph should hold
  by `EvidenceRef` rather than inline.
- Use `CheckpointStore` only for opaque checkpoint bytes. The schema belongs to
  the engine/run/optimizer layer that writes the checkpoint.
- Add shared error shape in `StoreError` only when more than one backend needs
  to report it. Backend-only validation should map into `OperationFailed` or a
  backend-local helper before crossing this trait boundary.

## Route Away
- Graph persistence codecs, checkpoint envelopes, restore laws, and graph
  admission belong in `leaven-engine`, with only opaque checkpoint bytes crossing
  this boundary.
- Product default store wiring belongs in `leaven-run`; do not make this crate
  choose inline, file, object, or sqlite defaults.
- Filesystem paths, JSON filenames, pointer files, object keys, sqlite schema,
  and retry policy that depends on a backend belong in the backend crate.
- Large evidence vocabulary belongs in `leaven-evidence` or the problem crate.
  This crate only stores values implementing the cold marker trait.

## Decision Cards
- when: changing a storage trait
  do: update at least one real backend in the same change, preferably file or
    inline, so the trait is proven by implementation
  preserve: store capability traits knowing refs and evidence markers, not graph
    records or product defaults
  avoid: adding methods just because `leaven-store-file` currently needs a
    convenience wrapper
  verify: run `cargo test -p leaven-store` plus the touched backend test

- when: adding checkpoint persistence
  do: keep the trait byte-oriented and place the typed schema at the writer
  preserve: engine/run ownership of restore laws and graph admission
  avoid: introducing `RunGraph` snapshots or optimizer-specific envelopes here
  verify: run the backend checkpoint tests and
    `cargo test -p leaven --test topology_contract`

## Local Bait
- The word `CheckpointStore` is not permission to introduce a graph snapshot
  model here. Keep checkpoint payloads byte-oriented until engine/run owns a
  schema.
- `StoreError::Serialization` is the shared boundary error; backend-specific
  parser details should be mapped into it instead of leaking backend types.
- Keep `src/lib.rs` as a map of modules and re-exports. New behavior belongs in
  the module that owns the capability.
- `BlobRef` and `EvidenceRef` share a store/key shape but are intentionally
  distinct. Do not collapse them to simplify a backend; access policy and graph
  semantics differ.

## Proof Anchors
- `cargo test -p leaven-store` proves the neutral trait crate still compiles
  without backend dependencies.
- `cargo test -p leaven --test topology_contract` proves this crate depends
  only on `leaven-core` and `leaven-kernel`, and that backend crates depend
  inward on `leaven-store`.
- When changing a trait or error variant, also run the touched backend test
  crate because the capability proof is in implementors, not in this crate
  alone.
