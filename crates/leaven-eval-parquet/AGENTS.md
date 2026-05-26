## Boundary
This crate owns physical Parquet row materialization into `leaven-eval` source-row manifests.

It may know Arrow and Parquet schemas, batches, nullability, and format-level read failures. It must not own benchmark split policy, paper-specific schema semantics, evaluator execution, proposer feedback, graph admission, or provider/runtime behavior.

## Routing
- `src/lib.rs` is a map only.
- `src/reader.rs` owns Parquet file reads, file-byte fingerprinting, row cursors, and typed column extraction helpers used by paper loaders.

## Local Bait
- A successful Parquet read is input materialization, not benchmark provenance. Paper dossiers still need source pins, dataset hashes, split manifests, and scorer/judge configuration before claiming replication.
- Column helpers expose physical values only. Paper examples must decide which columns are source ids, inputs, targets, categories, or judge metadata.

## Verification
- Run `cargo test -p leaven-eval-parquet` for row materialization behavior.
- Run `cargo test -p leaven --test topology_contract` when adding or changing dependencies.
