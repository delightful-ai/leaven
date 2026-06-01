## Boundary
This crate owns shape-specific agentic adapters for `GitProgramArtifact`.

It materializes durable Git program artifacts at commit revisions into
disposable workspaces and reads workspace Git mutations or explicit
`output/proposal.patch` / `output/proposal.bundle` artifacts back into typed
artifact changes. It may know `leaven-engine`, `leaven-artifact-git`,
`leaven-workspace`, and `leaven-workspace-git`.

It must not own Git artifact identity, generic workspace backend contracts,
optimizer frontier admission, scoring policy, Firkin product-pod mechanics, or
provider-specific agent protocol details.

## Local Bait
- A workspace checkout is not artifact identity. Readback must produce typed
  `GitProgramChange` values only after concrete revisions are imported into the
  durable store.
- Repo-backed AgentKit materialization may compose Git program checkout with an
  AgentKit profile projection. This crate may participate in the Git checkout
  and typed `GitProgramChange` readback path, but Codex CLI flags, app-server
  config, system-prompt channel lowering, and provider protocol details stay in
  provider leaves or the AgentKit profile adapter.
- Keep hidden/evaluator-only visibility policy outside this crate unless a
  materialization request explicitly carries it.
- Do not mount durable bare stores into proposer workspaces as a visibility
  boundary. Materialize disposable checkouts.
- Output proposals are import formats, not admission decisions. This crate may
  turn a patch or bundle into an imported child commit; graph advancement and
  score comparison still belong above it.
- `GitRevision::Tree` is artifact vocabulary, not a supported adapter input
  here. This crate rejects non-commit revisions explicitly until it owns a real
  tree export/materialization/readback flow.

## Verification
- `cargo test -p leaven-agentic-git` proves Git program materialization,
  commit-only contract rejection, checkout readback, and output patch/bundle
  proposal readback behavior over `leaven-workspace-local`.
