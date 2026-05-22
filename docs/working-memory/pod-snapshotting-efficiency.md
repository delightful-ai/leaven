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

Next concrete actions:

- Inspect the Firkin live snapshot route only after artifact-native
  reconstruction has a stronger denominator; do not use a restored VM snapshot
  as artifact truth.
- Decide whether the goal should add a Firkin no-spend contract/proxy benchmark
  now, or leave Firkin runtime snapshot restore as a follow-up behind the
  optional backend feature.
