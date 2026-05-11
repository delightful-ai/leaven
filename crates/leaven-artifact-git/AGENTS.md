## Boundary
This crate is the Git artifact adapter placeholder. It may eventually own Git
artifact identity, diffs, filesystem changes, and Git-specific surfaces.

The current public structs and enums are shape reservations. They are not a
Git backend, not a workspace implementation, and not proof of VCS artifact
editing.

## Local Bait
- `GitArtifactIdentityMode::{Commit, Tree}` names a future semantic choice; do
  not route repository discovery, worktree lifecycle, or command execution here.
  Those belong in workspace/backend crates.
- `GitAgentKitSurface` and `GitSkillFrontmatterSurface` must not grow generic
  skill-bank behavior; reusable skill artifact rules live in
  `leaven-artifact-skill`.

## Verification
- `cargo check -p leaven-artifact-git` proves only that placeholder exports
  resolve.
- Real behavior needs fixture-backed Git artifact tests plus
  `cargo test -p leaven --test topology_contract` if dependencies or facade
  exposure change.
