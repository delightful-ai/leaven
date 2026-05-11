## Boundary
This crate is the future Git workspace backend for neutral `leaven-workspace`
capabilities.

Current public names are scaffolding. `GitWorkspaceFactory` does not yet prove
clone/worktree lifecycle, command execution, cleanup, or identity semantics.

## Local Bait
- Git artifact identity belongs in `leaven-artifact-git`; workspace checkout
  lifecycle belongs here. Do not merge those because both mention Git.
- Repository discovery and host command details must adapt to
  `leaven-workspace` instead of changing neutral path/lease contracts.

## Verification
- `cargo check -p leaven-workspace-git` proves only scaffold exports.
- Real behavior needs fixture-backed repo lifecycle tests plus topology checks
  if artifact or store dependencies are introduced.
