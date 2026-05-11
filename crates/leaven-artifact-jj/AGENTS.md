## Boundary
This crate is the Jujutsu artifact adapter placeholder. It may eventually own
JJ change/operation/conflict artifact vocabulary and JJ-specific surfaces.

The current public types are reservations. They do not prove JJ operation-log
handling, conflict parsing, or workspace execution.

## Local Bait
- Keep JJ artifact identity separate from repo workspace mechanics. Running
  `jj`, managing working copies, and command sandboxing belong in workspace or
  tooling layers, not artifact vocabulary.
- `JjConflictSurface` should model artifact projection when it becomes real; it
  should not absorb generic conflict-resolution workflow or engine mutation.

## Verification
- `cargo check -p leaven-artifact-jj` proves only that placeholder exports
  resolve.
- Real behavior needs deterministic JJ fixture tests plus
  `cargo test -p leaven --test topology_contract` if dependencies or facade
  exposure change.
