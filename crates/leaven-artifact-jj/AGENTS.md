## Boundary
This crate owns Jujutsu artifact vocabulary for materialized file snapshots.

`JjArtifact` is a behavior-bearing file snapshot artifact. It writes files into
workspace slots, reads back `.leaven/jj/change.patch` as a `JjChange::Patch`,
and derives content/cache identity from the file map.

JJ operation-log handling, conflict parsing, surface projection, and workspace
execution are not implemented here. Empty reservation types for those concepts
were removed; reintroduce them only with fields, behavior, and contract tests.
No crate-level `JjArtifactError` exists until a real fallible JJ artifact
operation needs an owning error surface.

## Local Bait
- Keep JJ artifact identity separate from repo workspace mechanics. Running
  `jj`, managing working copies, and command sandboxing belong in workspace or
  tooling layers, not artifact vocabulary.
- A future `JjConflictSurface` should model artifact projection when it becomes
  real; it should not absorb generic conflict-resolution workflow or engine
  mutation.
- Do not add placeholder error enums for future JJ operations. Add the operation
  first, then add the smallest error type that describes its actual failures.

## Verification
- `cargo test -p leaven-artifact-jj --test materializable` proves current file
  snapshot materialization, content/cache identity, patch readback, absent patch
  handling, and invalid UTF-8 refusal.
- Operation-log, conflict, and surface behavior need deterministic JJ fixture
  tests plus
  `cargo test -p leaven --test topology_contract` if dependencies or facade
  exposure change.
