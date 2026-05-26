## Boundary
This crate owns Git artifact vocabulary: normalized repository paths,
immutable object ids, branch/tag ref identity, typed ref lineage, filesystem
changes, and diffs.

`GitArtifact`, `GitRef`, `GitLineage`, and `GitChange` are behavior-bearing
data shapes. They model content-addressed candidate state and branch/tag
membership for optimizer artifacts such as EvoSkill program/frontier snapshots.
They do not execute Git commands.

## Local Bait
- `GitArtifactIdentityMode::{Commit, Tree}` names a future semantic choice; do
  not route repository discovery, worktree lifecycle, or command execution here.
  Those belong in workspace/backend crates.
- Empty surface marker structs such as the old `GitPathSurface`,
  `GitAgentKitSurface`, and `GitSkillFrontmatterSurface` must stay absent until
  they implement real surface behavior with contract tests. Reusable skill
  artifact rules live in `leaven-artifact-skill`.
- Frontier/admission decisions belong in optimizer or population crates. This
  crate may record a `frontier/*` tag ref after a strategy decides it exists,
  but it must not decide whether the candidate enters the frontier.

## Verification
- `cargo nextest run -p leaven-artifact-git` proves Git path/ref/object
  validation, content/cache identity, ref lineage, and discarded-candidate ref
  cleanup over fixture-backed artifact state.
- Run `cargo test -p leaven --test topology_contract` if dependencies or facade
  exposure change.
