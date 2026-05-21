## Boundary
This crate owns shape-specific agentic adapters for `GitProgramArtifact`.

It materializes durable Git program artifacts into disposable workspaces and
reads workspace Git mutations back into typed artifact changes. It may know
`leaven-engine`, `leaven-artifact-git`, `leaven-workspace`, and
`leaven-workspace-git`.

It must not own Git artifact identity, generic workspace backend contracts,
optimizer frontier admission, scoring policy, Firkin product-pod mechanics, or
provider-specific agent protocol details.

## Local Bait
- A workspace checkout is not artifact identity. Readback must produce typed
  `GitProgramChange` values only after concrete revisions are imported into the
  durable store.
- Keep hidden/evaluator-only visibility policy outside this crate unless a
  materialization request explicitly carries it.
- Do not mount durable bare stores into proposer workspaces as a visibility
  boundary. Materialize disposable checkouts.

## Verification
- `cargo test -p leaven-agentic-git` proves Git program materialization and
  readback behavior over `leaven-workspace-local`.
