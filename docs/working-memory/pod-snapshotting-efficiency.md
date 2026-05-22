# Pod Snapshotting Efficiency

Active goal: figure out and implement the lightest real snapshot/restore path
that can reconstruct every intermediate artifact state, especially Git artifacts
from evolutionary/agentic runs, with Firkin as the sandbox backend candidate.

## 2026-05-22 Initial Handoff And Local Chain Proof

Primary handoff:

- `docs/plans/2026-05-22-pod-snapshotting-efficiency/goal-handoff.yaml`

Current code evidence:

- `xtask/src/git_trust_bench.rs` has `--intermediate-count N`.
- `xtask/AGENTS.md` documents that mode.
- jj commit: `qpxymzry` / `dd10dff4`
  `pod-snapshotting: add intermediate Git reconstruction benchmark`.

Reports generated under `target/git-trust-lane/`:

- `pod-snapshot-baseline-probe.json`
  - command: `cargo run -p xtask -- git-trust-bench --skip-trust-tests --iterations 1 --jobs 2 --case tiny-30x-probe:3:1024 --out target/git-trust-lane/pod-snapshot-baseline-probe.json`
  - result: one local parent->child sample passed; readback mean `0.219587416s`.
- `pod-snapshot-baseline-30x-local.json`
  - command: `cargo run -p xtask -- git-trust-bench --skip-trust-tests --iterations 30 --jobs 8 --case tiny-30x-local:3:1024 --out target/git-trust-lane/pod-snapshot-baseline-30x-local.json`
  - result: thirty independent local samples passed; readback mean
    `0.3663439041666667s`, readback p95 `0.405851583s`.
- `pod-snapshot-intermediate-chain-30.json`
  - command: `cargo run -p xtask -- git-trust-bench --skip-trust-tests --iterations 1 --jobs 1 --case tiny-intermediate-chain:3:1024 --intermediate-count 30 --out target/git-trust-lane/pod-snapshot-intermediate-chain-30.json`
  - result: one local chain with thirty child revisions passed. Every restored
    revision matched `HEAD` and content marker checks.
  - observed metrics: save total `7.824573879s`, save mean `0.2608191293s`,
    restore total `2.68220675s`, restore mean `0.08940689166666667s`,
    restore max `0.098101s`, changed bytes `1290`, durable growth `604 KiB`,
    coarse storage amplification `479.4542635658915`.

Verification run:

- `cargo check -p xtask`
- `cargo fmt --check`
- `cargo clippy -p xtask -- -D warnings`
- `cargo run -p xtask`
- `git diff --check`

Important limitation:

- This is a local artifact-native Git reconstruction proof, not a Firkin runtime
  snapshot proof.
- The 30x benchmark report is still single-repo.
- Storage accounting uses coarse `du -sk`, so amplification is useful as
  first-pass evidence, not a final storage model.

## 2026-05-22 Multi-Repo Artifact Reconstruction Proof

Code evidence:

- `crates/leaven-agentic-git/tests/git_program_materializer.rs` now has
  `readback_children_rematerialize_every_multi_repo_intermediate`.
- jj commit: `uqstssov` / `02da66c5`
  `pod-snapshotting: prove multi-repo intermediate restoration`.

Behavior proven:

- Start from one `GitProgramArtifact` with two repos: `program` and `bench`.
- For three sequential intermediate steps:
  - materialize the parent artifact into a workspace;
  - mutate both repo worktrees;
  - read back one atomic `GitProgramChange::AdvanceRepos`;
  - apply the change through `Artifact::apply_change`;
  - rematerialize the child artifact into a fresh workspace;
  - assert both repo files exactly match that intermediate's expected content.

Verification run:

- `cargo test -p leaven-agentic-git readback_children_rematerialize_every_multi_repo_intermediate -- --nocapture`

Companion artifact policy:

- Current implementation evidence is intentionally Git-artifact-native:
  `GitProgramArtifact` owns one-or-many repo revisions and layout.
- File, text, prompt, and other non-Git companions are separate artifacts under
  the existing `leaven_core::Artifact` law. This goal does not claim arbitrary
  mixed-artifact reconstruction until those artifact types have their own
  durable identity plus materialization/readback proof.
- Do not add a generic "codebase" wrapper or proxy companion state just to make
  the Git proof look broader. Multiple repos are already native to the Git
  artifact value; mixed artifact composition remains future work unless a real
  product artifact type needs it.

## 2026-05-22 Firkin Boundary Audit

Current Leaven/Firkin state:

- `crates/leaven-workspace-firkin` is present and optional.
- `crates/leaven/Cargo.toml` exposes it through `workspace-firkin`; it is not a
  default feature and is not routed through `leaven::prelude`.
- `FirkinWorkspaceRuntime` owns workspace allocation, file operations, command
  execution, and cleanup. It does not expose snapshot save/restore as a Leaven
  workspace operation.
- `firkin_product_pod_materializes_and_reads_back_isolated_git_workspaces`
  proves two workspace allocations share one product pod id while using
  different container ids and workspace roots; Git materialization/readback
  stays isolated between them.

Firkin runtime snapshot facts from
`/Users/darin/vendor/github.com/apple/containerization`:

- `crates/runtime/src/continuation.rs` has
  `RuntimeContinuationSnapshotCapture` and
  `RuntimeContinuationSnapshotRestore`.
- `CoreContainerSnapshotSink` saves VM/container snapshot bytes plus persisted
  restore state.
- `RuntimeSnapshotRestore` records `warm_snapshot_restore`; continuation
  capture records `snapshot_save`.
- `crates/runtime/tests/live_snapshot_restore.rs` has ignored signed Apple/VZ
  tests for snapshot restore, continuation restore, product-route snapshot
  restore, and restore timing artifacts.

Decision for this goal:

- Git intermediate reconstruction should not use Firkin snapshots in the hot
  path. Durable Git object import plus `GitProgramArtifact` revisions are
  lighter, already measured at 30 intermediates, and reconstruct exact artifact
  states without restoring a VM/rootfs.
- Firkin snapshots are still useful for runtime continuation: preserving
  process state, warm pools, expensive dependency setup, or an interactive
  agent session that is not recoverable from artifact-native state.
- Therefore Leaven should keep Firkin snapshot support out of the artifact law.
  If a later backend wants runtime continuation, add it as an explicit runtime
  capability/report, with `snapshot_save`/`warm_snapshot_restore` measurements
  and signed live proof.

Verification run:

- `cargo test -p leaven-workspace-firkin`
- `cargo test -p leaven --no-default-features --features workspace-firkin --test public_surface_contract`

## 2026-05-22 Verification Closeout

Fresh verification after the multi-repo proof and Firkin boundary docs:

- `cargo fmt --check`
- `ruby -e 'require "yaml"; YAML.load_file("docs/plans/2026-05-22-pod-snapshotting-efficiency/goal-handoff.yaml"); puts "yaml ok"'`
- `git diff --check`
- `cargo test -p leaven-agentic-git readback_children_rematerialize_every_multi_repo_intermediate -- --nocapture`
- `cargo test -p leaven-workspace-firkin`
- `cargo test -p leaven --no-default-features --features workspace-firkin --test public_surface_contract`
- `cargo clippy -p leaven-agentic-git --tests -- -D warnings`
- `cargo clippy -p leaven-workspace-firkin --tests -- -D warnings`

Goal-scope conclusion:

- For Git-backed evolutionary/agentic intermediates, the lightest proven path is
  artifact-native Git object import/readback plus `GitProgramArtifact` revision
  identity.
- Firkin product pods are the execution backend boundary. They provide isolated
  workspace containers under one product pod and compose with the existing Git
  materializer/readback path.
- Firkin VM/container snapshots are not on the Git artifact hot path. They
  should be added only as explicit runtime continuation support when preserving
  process/session state matters more than reconstructing artifact state.
- Non-Git companion artifacts remain intentionally unclaimed future work unless
  a real artifact type includes them in identity/materialization/readback law.

Next concrete actions:

- If this goal is extended beyond Git artifacts, pick one real companion
  artifact type and implement its own artifact-native reconstruction proof
  instead of widening the Git proof by assertion.
