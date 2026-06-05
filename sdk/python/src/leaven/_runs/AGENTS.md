## Boundary

`leaven._runs` owns Rust-owned run readback bridging for the current Python SDK
foundation slice. It may list local run directories that carry Rust checkpoint
state, invoke `leaven run inspect --run-dir ...` to project Rust-owned
checkpoint/graph readback into `lv.runs.open(...)` and `lv.runs.inspect(...)`,
and invoke
`leaven run blob --run-dir ... --store ... --key ...` to retrieve bytes for
Rust-owned blob refs exposed by that inspection export, and invoke
`leaven run evidence --run-dir ... --store ... --key ...` to retrieve bytes
for Rust-owned evidence refs exposed by that inspection export.

It must not write or read Python-only run-result projections such as
`optimized.json`, spawn the seam server, own optimizer strategy, interpret
provider protocols, or become the public inspection namespace.

## Public Dependencies

- Public SDK result and artifact records.
- Python standard-library filesystem module.
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

- `rust_evidence.py`: typed Python projection from Rust-owned
  `CaseAssessmentEvidence` byte exports into `RunInspection` summaries. It may
  decode the Rust serde shape for SDK inspection, but it must not define new
  Rust evidence semantics or synthesize reward-vector rows that Rust did not
  persist.
- `rust_export.py`: private subprocess bridge to Rust-owned checkpoint/graph
  inspection, run-blob byte exports, and evidence byte exports.
- `store.py`: deterministic Rust-checkpoint run-directory listing.
- `__init__.py`: private map-only re-export.
