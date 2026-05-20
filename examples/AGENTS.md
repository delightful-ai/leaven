## Boundary
This subtree contains executable milestone packages. They are checks for named Leaven surfaces, not snippets that may drift from the library contract. The classification below controls whether a package is product proof, mechanics smoke, or only a proxy demo.

Each `pN_*` directory is a workspace package with a runnable binary. Keep reusable behavior in the owning library crate; examples may define tiny local artifacts, proposers, evaluators, fixtures, and scripts only when they are part of the acceptance scenario.

## Package Map
- `p0_graph_skeleton`: product-proof for the graph skeleton milestone; `just milestone-p0` proves seed/create/change graph basics through `RunContext`.
- `p1_keep_best`: product-proof for scalar keep-best; `just milestone-p1` proves scalar evaluation, inline evidence storage, and keep-best selection.
- `p2_pairwise_tournament`: product-proof for pairwise tournament plumbing; `just milestone-p2` proves pairwise request/evidence flow and fitted tournament selection.
- `p3_gepa_parity`: mechanics-smoke for GEPA-shaped loop plumbing; `just milestone-p3` proves proposal/evaluation over an explicit edit surface and casewise frontier, but the current fixed-edit reflection path is not product GEPA reflection.
- `p4_meta_harness_lite`: product-proof for the P4 workspace/trust milestone; `just milestone-p4` proves materialized workspace history, create proposals, hidden test filtering, evidence refs, and cleanup.
- `p5_evoskill_iteration`: local `AGENTS.md`; `just milestone-p5` is a live Codex/EvoSkill proof and is not cheap or deterministic.
- `p6_optimizer_policy_self_opt`: product-proof for trust-policy self-optimization; `just milestone-p6` proves optimizer-policy self-optimization over hidden validation/test partitions and hidden-test refusal.
- `p7_self_optimization_kernel`: product-proof for the promotion-gate milestone; `just milestone-p7` proves immutable public surfaces, hidden holdout refusal, promotion gates, final-test selection, and rollback metadata.
- `p8_aime_gepa`: local `AGENTS.md`; `just milestone-p8` is a product-proof for the public builder path through LM-backed GEPA reflection over provider-neutral `leaven-lm`, with deterministic local model output. It is not proof of a live provider, LM cache behavior, or live AIME improvement.
- `memento_skills_read_write`: local `AGENTS.md`; paper-specific Memento-Skills tiny live Read-Write proof. It is outside the Cargo workspace and writes live artifacts under `tmp/memento_skills_read_write/`.
- `skillreducer_tiny`: local `AGENTS.md`; paper-specific SkillReducer tiny live debloating proof. It is outside the Cargo workspace and writes live artifacts under `tmp/skillreducer_tiny/`.
- `d2skill_tiny`: local `AGENTS.md`; paper-specific D2Skill tiny live paired-rollout skill-bank proof. It is outside the Cargo workspace and writes live artifacts under `tmp/d2skill_tiny/`.
- `trace2skill_tiny_live`: local `AGENTS.md`; paper-specific Trace2Skill tiny live trajectory-to-skill proof. It is outside the Cargo workspace and writes live artifacts under `tmp/trace2skill_tiny_live/`.
- `trace2skill_spreadsheetbench`: mechanics-smoke for Trace2Skill's official
  SpreadsheetBench-Verified 400-row manifest and run-artifact lowering. `cargo
  test -p trace2skill_spreadsheetbench --test manifest` proves the local
  upstream JSON is parsed into `leaven-eval` cases and the paper's `0..200`
  train / `200..400` held-out split. `cargo test -p
  trace2skill_spreadsheetbench --test run_artifacts` proves upstream-shaped
  `results.json`, logs, and analysis reports can be imported into
  `AgentTrajectoryCorpusEvidence`. `cargo test -p
  trace2skill_spreadsheetbench --test patch_bridge` proves upstream-shaped
  fenced JSON patches lower into `SkillPatchPlan` plus concrete
  `SkillBankChange` values and apply through `SkillPatchApplication`. `cargo
  test -p trace2skill_spreadsheetbench --test patch_replay` proves saved/live
  JSON patch merge artifacts and upstream-shaped `--save-intermediates`
  directories can replay through `SkillPatchMergeTree` and
  `SkillPatchApplication`. These do not prove trajectory generation,
  spreadsheet execution, model-backed analyst or merge calls, skill evolution,
  or metric reproduction.

## Proof Classification
- `product-proof`: an example that exercises the real public contract at the intended user layer, with no proxy substitution for the behavior being claimed.
- `mechanics-smoke`: an example that proves wiring, reporting, split handling, or topology while using a local fixture for the hard behavior.
- `proxy-demo`: an example that demonstrates a desired flow through a substitute implementation; useful for design pressure, but not evidence that the product behavior exists.

When adding or changing an example, classify it before citing it as acceptance evidence. Coverage, `just check`, and milestone execution can prove the example still runs; they do not by themselves promote a mechanics-smoke or proxy-demo into product-proof.

## Local Rules
- Do not add behavior to an example because the owning crate is missing it. Implement the primitive in the crate that owns the fact, then use it here.
- Deterministic examples must stay deterministic by default. Live network/provider/model paths must be explicit opt-ins with environment variables or CLI flags, and their AGENTS guidance must call out the spend.
- Do not let a fixture with a production-looking type name stand in for product behavior. If the example uses a fixed proposer, fake LM, scripted runner, or shell-out provider path, say exactly which product path it does not prove.
- Preserve train/validation/test and hidden holdout boundaries in examples that prove trust policy. Do not collapse them into one public case set to make the binary shorter.
- Keep generated data, provider cache material, workspaces, checkpoints, and run output under `target/` or `tmp/`; do not add committed fixtures unless they are small deterministic acceptance inputs.
- If an example needs local helper types, name them as local fixtures unless they are the public API under test. Public-looking helper names are the fastest way to turn a smoke test into a false product claim.
- Live modes must be swaps over the same Leaven path to count as product proof. A script or provider subprocess can be useful operator tooling, but it is a proxy-demo until solver/reflector/runtime/cache behavior flows through Leaven-owned traits or role configuration.

## Decision Cards
- when: adding a new milestone package
  do: put reusable behavior in the owning crate first, classify the example, and add a `just milestone-pN` recipe
  preserve: deterministic default execution unless the point of the package is explicitly live
  avoid: committed provider caches, hidden network calls, or example-only primitives that look like library API
  verify: run the milestone recipe and update `docs/testing/README.md`

- when: upgrading a mechanics-smoke to product-proof
  do: replace fixtures/proxies with the intended public Leaven path before changing the label
  preserve: the original cheap smoke if it still catches useful wiring regressions
  avoid: reclassifying because coverage, `just check`, or benchmark numbers are green
  verify: add or update the proof in `docs/testing/README.md` and run the relevant milestone plus `just check`

## Verification
- For one example, use its `just milestone-pN` command and read the package map above for what that command proves.
- `just milestone-examples` expands through `milestone-p5`, and `milestone-p5` sets `LEAVEN_CODEX_LIVE=1`; do not cite `just milestone-examples` as a cheap deterministic proof.
- For shared example behavior, run the affected deterministic `just milestone-pN` commands and explicitly decide whether the live p5 proof is required. Final behavior gate remains `just check` when the change is not documentation-only.
