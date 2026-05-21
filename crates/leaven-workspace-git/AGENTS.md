## Boundary
This crate owns Git checkout lifecycle for neutral `leaven-workspace`
capabilities.

`GitWorkspaceFactory` can clone a local source repository into an isolated
workspace, optionally check out a named ref, expose a local mount, run workspace
commands, and clean up the checkout. `GitCheckout` can capture tracked files
and refs into `leaven-artifact-git` records, restore a named ref, and delete a
branch or tag ref. This crate does not own artifact identity or optimizer
admission semantics.

## Local Bait
- Git artifact identity belongs in `leaven-artifact-git`; workspace checkout
  lifecycle belongs here. Do not merge those because both mention Git.
- Repository discovery and host command details must adapt to
  `leaven-workspace` instead of changing neutral path/lease contracts.
- Keep capture/readback explicit; do not hide Git branch/tag mutation behind
  generic file writes.
- EvoSkill score metadata, parent/child admission, and checkpoint semantics
  belong in the paper harness, optimizer/population, or run-store layers. This
  crate only performs checkout/ref operations.

## Verification
- `cargo nextest run -p leaven-workspace-git` proves local clone, checkout,
  command execution, local mount exposure, capture/readback, restore, branch/tag
  deletion, and cleanup over a fixture Git repo.
- Run `cargo test -p leaven --test topology_contract` if artifact or store
  dependencies are introduced.
