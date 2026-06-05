## Boundary

`leaven._runs` owns private on-disk run-result persistence and Rust-owned
readback bridging for the current Python SDK foundation slice. It writes and
reads the SDK's inspectable `Optimized` projection so `lv.runs.open(...)` can
inspect a completed run after the original Python process exits, and it may
invoke `leaven run inspect --run-dir ...` to attach Rust-owned checkpoint/graph
readback to `lv.runs.inspect(...)`, and invoke
`leaven run blob --run-dir ... --store ... --key ...` to retrieve bytes for
Rust-owned blob refs exposed by that inspection export, and invoke
`leaven run evidence --run-dir ... --store ... --key ...` to retrieve bytes
for Rust-owned evidence refs exposed by that inspection export.

It may know the private JSON file layout and artifact codecs needed to
round-trip current SDK result objects. It must not spawn the seam server, own
optimizer strategy, interpret provider protocols, or become the public
inspection namespace.

## Public Dependencies

- Public SDK result and artifact records.
- Python standard-library filesystem and JSON modules.
- The installed/built `leaven` CLI public command:
  `leaven run inspect --run-dir <path>`.
- The installed/built `leaven` CLI public command:
  `leaven run blob --run-dir <path> --store <store> --key <key>`.
- The installed/built `leaven` CLI public command:
  `leaven run evidence --run-dir <path> --store <store> --key <key>`.

## Private Dependencies

- Sibling modules inside `leaven._runs`.
- Public `leaven.run_inspection` records for typed Rust readback projection.
- No imports from `leaven._seam` or `_seam_optimize`. `leaven._serve` has been
  removed and must not return as an inspection dependency.

## Map

- `codec.py`: JSON envelope and current artifact round-trip codecs.
- `rust_export.py`: private subprocess bridge to Rust-owned checkpoint/graph
  inspection, run-blob byte exports, and evidence byte exports.
- `store.py`: deterministic run-directory write/read/list operations.
- `__init__.py`: private map-only re-export.
