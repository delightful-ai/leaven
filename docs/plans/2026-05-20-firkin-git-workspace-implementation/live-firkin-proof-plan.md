# Live Firkin Git Workspace Proof Plan

Status: active implementation plan
Date: 2026-05-21

This plan extends the Firkin Git workspace implementation without changing its
scope. The previous closeout proved the Leaven contract shape with a
host-backed product-pod runtime. This slice must prove the same Git
materialization/readback behavior against a real Firkin product pod without
depending on host-local repository paths or `Workspace::local_mount()`.

## Current Denominator

The existing contract e2e in
`crates/leaven-workspace-firkin/tests/firkin_git_e2e.rs` proves two isolated
workspace allocations in one product pod and dirty-worktree readback through
the Firkin workspace backend. It does not prove the live Apple/VZ adapter path.

Inspection found a real blocker: `GitProgramMaterializer` currently configures
workspace checkouts with a remote whose URL is the host bare-repo path from
`GitProgramStores`. That is acceptable only for local or host-backed fake
workspaces. A real product-pod container cannot read that host path, so a live
test would fail for the right reason.

## Implementation Shape

1. Add a focused `leaven-agentic-git` test that uses a no-local-mount workspace
   and rejects any workspace command argument containing a host durable-store
   path. The current code should fail this test because it passes the host bare
   repo path to `git remote add origin`.
2. Change materialization to create a host-side Git bundle from the durable
   store for the requested commit, write that bundle into the workspace through
   `WorkspaceView::write_file`, fetch from the guest-visible bundle path, check
   out the requested commit, and remove the temporary bundle.
3. Keep readback as the mirror image: workspace-created bundle bytes are read
   through `WorkspaceView` and imported into the durable store after validation.
4. Add an optional `firkin-apple-vz-live` feature to
   `leaven-workspace-firkin`, depending on Firkin's `single-node` crate only
   when live proof is requested.
5. Add an ignored live integration test that starts a real Apple/VZ product
   pod, allocates two Leaven workspaces in that pod, materializes the Git
   artifact in both, mutates one workspace, reads back/imports the child commit,
   verifies the other workspace stayed at the parent, and stops the pod.
6. Add a signed runner script that builds the ignored live test, signs the test
   binary with Firkin's VZ entitlement file, and runs the exact test.

## Completion Evidence

Narrow gates:

```bash
cargo test -p leaven-agentic-git --test git_program_materializer -- --nocapture
CARGO_TARGET_DIR=/tmp/leaven-firkin-target CARGO_BUILD_JOBS=1 cargo test -p leaven-workspace-firkin --features firkin-facade --test firkin_git_e2e -- --nocapture
CARGO_TARGET_DIR=/tmp/leaven-firkin-target CARGO_BUILD_JOBS=1 cargo test -p leaven-workspace-firkin --features firkin-apple-vz-live --test firkin_live_git_e2e --no-run
git diff --check
```

Live proof gate:

```bash
LEAVEN_FIRKIN_LIVE_TEMPLATE_IMAGE=<image-with-git-and-sh> \
  scripts/run-signed-live-firkin-git-workspace-test.sh
```

Expected live pass signal:

- The ignored test boots one live product pod.
- Both Leaven workspaces report `local_mount() == None`.
- Both workspaces materialize the same parent commit from workspace-visible Git
  bundles, not host bare-repo remotes.
- Workspace A mutation imports a child commit into the durable store.
- Workspace B still reads the parent file content.
- Workspace cleanup removes containers and the product pod is stopped.

Residual risk: the live gate depends on local Apple/VZ availability, signing
entitlements, and an OCI image that contains `git`, `sh`, `cat`, `find`,
`mkdir`, `rm`, `test`, and `sleep`.
