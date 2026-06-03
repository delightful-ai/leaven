## Boundary

`leaven._runs` owns private on-disk run-result persistence for the current
Python SDK foundation slice. It writes and reads the SDK's inspectable
`Optimized` projection so `lv.runs.open(...)` can inspect a completed run after
the original Python process exits.

It may know the private JSON file layout and artifact codecs needed to
round-trip current SDK result objects. It must not spawn the seam server, own
optimizer strategy, interpret provider protocols, or become the public
inspection namespace.

## Public Dependencies

- Public SDK result and artifact records.
- Python standard-library filesystem and JSON modules.

## Private Dependencies

- Sibling modules inside `leaven._runs`.
- No imports from `leaven._seam`, `_seam_optimize`, or `_serve`.

## Map

- `codec.py`: JSON envelope and current artifact round-trip codecs.
- `store.py`: deterministic run-directory write/read/list operations.
- `__init__.py`: private map-only re-export.
