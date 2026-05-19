## Boundary
This crate owns Git checkout lifecycle for neutral `leaven-workspace`
capabilities.

`GitWorkspaceFactory` can clone a local source repository into an isolated
workspace, optionally check out a named ref, expose a local mount, run workspace
commands, and clean up the checkout. It does not own Git artifact identity or
optimizer admission semantics.

## Local Bait
- Git artifact identity belongs in `leaven-artifact-git`; workspace checkout
  lifecycle belongs here. Do not merge those because both mention Git.
- Repository discovery and host command details must adapt to
  `leaven-workspace` instead of changing neutral path/lease contracts.
- Capture/readback of a checkout into `GitArtifact` records is the next bridge
  seam. Keep it explicit; do not hide Git branch/tag mutation behind generic
  file writes.

## Verification
- `cargo nextest run -p leaven-workspace-git` proves local clone, checkout,
  command execution, local mount exposure, and cleanup over a fixture Git repo.
- Run `cargo test -p leaven --test topology_contract` if artifact or store
  dependencies are introduced.
