## Boundary
This crate owns Jujutsu artifact vocabulary and JJ-specific surfaces.

`JjTrackedRun`, `JjSnapshotRecord`, `JjEvaluationRecord`, and
`JjSnapshotPolicy` are behavior-bearing data shapes for durable goal/eval
tracking. They record what a higher layer captured; they do not execute `jj`.

The older change/operation/conflict/surface public types are still reservations.
They do not prove JJ operation-log handling, conflict parsing, or workspace
execution.

## Local Bait
- Keep JJ artifact identity separate from repo workspace mechanics. Running
  `jj`, managing working copies, and command sandboxing belong in workspace or
  tooling layers, not artifact vocabulary.
- `JjConflictSurface` should model artifact projection when it becomes real; it
  should not absorb generic conflict-resolution workflow or engine mutation.

## Verification
- `cargo check -p leaven-artifact-jj` proves only that placeholder exports
  resolve.
- `cargo test -p leaven-artifact-jj --test tracked_run` proves durable tracked
  run, snapshot, eval, and policy vocabulary.
- Real behavior needs deterministic JJ fixture tests plus
  `cargo test -p leaven --test topology_contract` if dependencies or facade
  exposure change.
