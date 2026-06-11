## Boundary
This crate owns shape-specific agentic adapters for `GitProgramArtifact`.

It materializes durable Git program artifacts at commit revisions into
disposable workspaces and reads workspace Git mutations or explicit
`output/proposal.patch` / `output/proposal.bundle` artifacts back into typed
artifact changes. It may know `leaven-engine`, `leaven-artifact-git`,
`leaven-workspace`, and `leaven-workspace-git`.

It also builds a single-repo `GitProgramArtifact` (plus its `GitProgramStores`)
from an in-memory `GitPath -> bytes` file map with a deterministic seed commit
(`build_program_seed`), and reads a revision's tracked files back into the same
flat map (`read_revision_files`). These let a host that owns only flat content
construct a real run-scoped store, run the agentic loop over the typed artifact,
and project an evolved child revision back to flat content. They are generic
Git-program operations: the deterministic commit identity is pinned host-git
plumbing (`hash-object`/`write-tree`/`commit-tree` with fixed author/committer
identity and date), not a worktree commit. The projection between a domain
artifact (such as an AgentKit wire record) and this flat file map belongs to the
domain layer that owns it, not here.

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
- Readback change detection is content-truthful, not stat-truthful. An agent
  rewrites a tracked file as a foreign process; a stat-based check (`git
  status`, `git diff-index`, even `git update-index --refresh`) can trust a
  colliding index stat and drop the edit as "no changes" — configuration- and
  race-dependent under `core.checkStat=minimal`, `core.fsmonitor`, or load.
  `worktree_differs_from_parent` re-hashes the worktree into a scratch index and
  compares tree object ids, and `freeze_worktree` re-stages with `add
  --renormalize -A` so the imported child carries the edited content, not the
  seed. Do not replace either with a plain stat-based status/add shortcut.

## Verification
- `cargo test -p leaven-agentic-git` proves Git program materialization,
  commit-only contract rejection, checkout readback, and output patch/bundle
  proposal readback behavior over `leaven-workspace-local`. The
  `git_program_seed` target proves seed construction round-trips flat content
  through a real revision, that identical content yields a deterministic seed
  commit id while different content diverges, empty file-set refusal, and that
  an evolved child revision reads back the changed content.
