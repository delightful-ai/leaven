## Boundary
`leaven-store-file` is the local filesystem backend for storage capabilities.
It owns directory layout, JSON serialization for typed convenience stores, local
key validation, and reopen behavior.

It depends inward on `leaven-store`; it must not change storage trait meaning,
choose product defaults, or define engine checkpoint schemas.

## Map
- `src/store.rs`: `FileStore`, the aggregate local filesystem store for blobs
  plus byte checkpoints. Its layout is `<root>/blobs/` and
  `<root>/checkpoints/`.
- `src/evidence.rs`: `FileEvidenceStore<E>` writes typed evidence as JSON files,
  `FileCheckpointStore` writes opaque checkpoint bytes, and
  `FileJsonCheckpointStore<T>` is a typed JSON convenience wrapper over byte
  checkpoints.
- `tests/file_stores.rs`: the local contract for reopen-without-overwrite,
  wrong-store rejection, invalid key rejection, latest checkpoint pointers,
  typed JSON checkpoints, and aggregate blob/checkpoint round trips.

## Local Helper Stack
- Use `FileStore::open_named` when a test or operator path needs stable store
  names in refs; default `FileStore::open` uses `"file"` for blob refs.
- Use `FileEvidenceStore<E>` when the evidence schema is caller-owned and
  serializable; keys are decimal JSON files and reopen resumes after the
  highest numeric key.
- Use `FileCheckpointStore` for opaque bytes and `FileJsonCheckpointStore<T>`
  only as a typed convenience over a caller-owned checkpoint schema.
- Keep key validation local: evidence keys are non-empty ASCII digits, blob keys
  cannot contain path separators, and `LATEST` is a pointer file rather than a
  checkpoint payload.

## Route Away
- Add or change storage traits in `leaven-store`, then update this backend as
  one implementor.
- Put in-memory defaults and cheap non-durable tests in `leaven-store-inline`,
  not by weakening this backend's filesystem behavior.
- Put object-store keys and sqlite schemas in their backend crates when those
  crates grow real behavior; do not pre-generalize file layout for them here.
- Keep graph checkpoint schema and restore semantics in `leaven-engine` or the
  caller-owned checkpoint type. This backend persists bytes and optional typed
  JSON wrappers only.

## Decision Cards
- when: changing file layout
  do: update `tests/file_stores.rs` to prove reopen, latest pointer, invalid key,
    wrong-store, and missing-payload behavior
  preserve: existing stores being append/reopen friendly unless the change is a
    deliberate hard cutover
  avoid: silently changing ref keys without a migration or explicit break
  verify: run `cargo test -p leaven-store-file --test file_stores`

- when: adding typed persistence convenience
  do: layer it over `CheckpointStore` or `EvidenceStore<E>` rather than changing
    the neutral trait for one JSON shape
  preserve: caller ownership of schemas
  avoid: making file JSON the default product checkpoint format
  verify: add focused tests to `tests/file_stores.rs`

## Local Bait
- The package description still says "skeleton", but this crate has real store
  behavior. Treat the source and `tests/file_stores.rs` as authoritative.
- Evidence schemas are problem-owned. Do not add one global evidence JSON shape
  here because this backend happens to serialize evidence values.
- Local root paths are backend details. Public callers should hold refs and use
  the `leaven-store` traits unless they are explicitly configuring this backend.
- Reopen scans skip non-numeric JSON files; that is operator tolerance, not a
  permission to store arbitrary sidecar schema in the evidence namespace.

## Proof Anchors
- `cargo test -p leaven-store-file --test file_stores` proves the concrete file
  layout, key rejection, reopen behavior, latest pointer, JSON checkpoint
  wrapper, and aggregate store behavior.
- `cargo test -p leaven --test topology_contract` proves this backend depends
  inward on `leaven-store` and does not pull graph/workspace crates into the
  store backend layer.
