# Leaven Testing Contract

Leaven uses tests to collapse the remaining implementation space after the
type, trait, and error surfaces are honest. Every test must name a real claim
and kill a plausible wrong implementation.

## Verification Lanes

Use the narrowest command set that proves the touched ownership surface before
claiming a narrow change is complete. For a small crate or docs slice, that is
usually the exact integration test, the owning crate test lane, targeted
clippy/fmt when Rust changed, and topology only if membership or dependency
edges changed.

Run the full local gate for broad shared behavior, workspace tooling or
coverage-floor changes, facade/default-feature/public-route changes,
release/PR readiness, or when a reviewer asks for workspace confidence:

```bash
just check
```

`just check` runs formatting, the production line-count lint, clippy with
workspace library/tool targets, the default workspace libtest suite, doctests,
and the line/branch coverage summary. The default gate excludes milestone example
packages; use the explicit milestone recipes when an example workflow is the
claim under test. Focused proof commands include:

```bash
just lint
just test
just test-one <cargo test args>
just test-stress 20 <cargo test args>
just build-incremental-canary
just coverage
just coverage-fast --package <crate>
just coverage-smoke-fast --package <crate> --test <integration-test-name>
just milestone-p0
just milestone-p1
just milestone-p2
just milestone-p3
just milestone-p4
just milestone-p5
just milestone-p6
just milestone-p7
just milestone-p8
just milestone-examples
```

The milestone examples are workspace packages under `examples/p*/`, not Cargo
example targets. They are intentionally excluded from default `just test`,
`just lint`, and `just coverage` compilation. `cargo check --workspace
--examples` is therefore not the proof command for them. Use the `just
milestone-*` recipes, which run each example binary directly.

Milestone execution is not automatically product proof. Classify examples as
product-proof, mechanics-smoke, or proxy-demo before citing them as acceptance
evidence. In particular, `just milestone-p8` now proves the public builder path
through target-safe AIME case lowering, source-id reporting, and LM-backed GEPA
reflection over provider-neutral `leaven-lm`; it does not prove concrete
provider transport, durable SQLite cache behavior, or live AIME improvement.

Current milestone classifications:

| Recipe | Classification | What a passing run proves |
| --- | --- | --- |
| `just milestone-p0` | product-proof for P0 graph skeleton | Seed/create/change graph basics through `RunContext`. |
| `just milestone-p1` | product-proof for scalar keep-best | Scalar evaluation, inline evidence storage, and keep-best selection. |
| `just milestone-p2` | product-proof for pairwise tournament plumbing | Pairwise request/evidence flow and fitted tournament selection. |
| `just milestone-p3` | mechanics-smoke | GEPA-shaped loop plumbing over an explicit edit surface and casewise frontier, not real evidence-aware reflection. |
| `just milestone-p4` | product-proof for P4 workspace/trust | Materialized workspace history, create proposals, hidden test filtering, evidence refs, and cleanup. |
| `just milestone-p5` | live product proof for the Codex/EvoSkill reproduction | Live Codex CLI execution, checkpointing, child skill bank construction, and summary output; it spends provider/runtime resources. |
| `just milestone-p6` | product-proof for trust-policy self-optimization | Hidden validation/test partition behavior and hidden-test refusal. |
| `just milestone-p7` | product-proof for promotion gates | Immutable public surfaces, hidden holdout refusal, final-test selection, and rollback metadata. |
| `just milestone-p8` | product-proof for LM-backed GEPA builder path | Public builder mechanics, target-safe AIME case lowering, source-id reporting, evidence-aware LM-backed reflection over `leaven-lm`, proposal recording, and candidate application. |

## Runtime SLA

The canonical full test suite has a wall-clock runtime target:

```text
just test execution should finish in <30s
```

`just test` reports this after compiling the workspace test binaries and
prewarming workspace doctests. Crossing 30s is a warning, not the hard failure
condition; failed subprocesses and the 600s timeout still fail the command.
Those preflight steps are still mandatory and must fail on compile errors, but
compiler wall time is not evidence that the runtime suite crossed the target.
The target covers default
workspace lib/bin/integration/example test targets through
`cargo test --workspace --all-targets` and workspace doctests for library/tool
packages that contain executable Rust doctest fences. Milestone examples stay
out of the default SLA and run through explicit
`just milestone-*` recipes. The current hard timeout is 600s so `just check`
still proves the suite completes while the 30s target remains visible. If the
suite crosses the target, do not add a second slow lane; reduce fixture cost,
property-test case count, setup work, doctest harness fan-out, or assertion
altitude until the default suite is back under the target.

## Build Policy Canary

Dev builds keep Cargo incremental compilation enabled. The workspace also uses
the nightly parallel rustc frontend through `.cargo/config.toml`; if a pinned
nightly regresses, the fallback is to remove that frontend flag before disabling
incremental. Use this canary before changing `profile.dev`, `.cargo/config.toml`,
or `rust-toolchain.toml`:

```bash
just build-incremental-canary
```

The canary runs the old incremental-plus-parallel-rustc ICE repro for
`trace2skill_spreadsheetbench` and the focused CLI/ACP/public-seam check under
`CARGO_INCREMENTAL=1`.

The SLA runner delegates workspace test discovery to Cargo, then executes the
discovered libtest binaries directly under the runtime deadline. Doctests are
run in a separate explicit lane, so the workspace discovery command uses
`--all-targets` to avoid running the doctest harness twice. That keeps compile
prewarm separate from measured execution without the external runner discovery
fan-out that can dominate this workspace's hot loop.

Coverage has hard failure floors plus warning targets. The current hard floors
are 80% line and 80% branch so coverage does not outrank executable seam
readiness work. The old ratchet values remain wired as `coverage_line_warn` and
`coverage_branch_warn`; treat those warnings as debt to retire with real tests,
not as a reason to block higher-priority V1 ACP proof.

For hot-loop coverage feedback, use `just coverage-fast --package <crate>` and
repeat `--package` for a small touched set. Add `--test <integration-test-name>`
when the changed proof is confined to one or two integration test targets. This
lane clears stale profraw files, reuses compiled `cargo-llvm-cov` artifacts,
and skips the `xtask` git-trust smoke binaries, so it is only an iteration aid.
It refuses non-default milestone packages and does not replace `just coverage`
or `just check` when full workspace/release confidence is required.

When the only question is whether the touched tests still pass under coverage
instrumentation, use `just coverage-smoke-fast --package <crate> --test
<integration-test-name>`. It skips report generation entirely and therefore
does not produce or enforce line/branch percentages.

The topology contract verifies the active workspace entrypoints and dependency
direction. It is not a maturity claim. The default coverage gate keeps hard
`coverage_line_floor` and `coverage_branch_floor` values over production/source
behavior after excluding non-default milestone packages from workspace
execution, and reports higher warning targets when the workspace falls below
the desired ratchet. It runs the workspace tests plus `xtask` under
`cargo llvm-cov` before reporting. The enforced denominator excludes test
harness files and `#[cfg(test)] mod ...` blocks after execution, so tests can
exercise production code without becoming code that must itself be covered.
Line and branch coverage are both enforced from the lcov report so generic
monomorphizations do not create duplicate missed-line denominators. Empty map
crates naturally add no executable denominator; once a crate gains runtime
behavior, that behavior is part of the canonical coverage surface and needs
contract tests in the same change. Coverage keeps the exercised surface honest;
it does not promote proxy examples or placeholder public names into mature
product contracts.

## Test Shapes

Use the narrowest layer that proves the claim.

- **Law tests** prove algebraic invariants over a space of inputs. Prefer
  `proptest` for type, graph, budget, cache-key, and resolution laws.
- **Example tests** pin a canonical input/output or error shape.
- **Scenario tests** drive a public surface across components, such as
  `RunContext` proposal/apply/evaluate flows.
- **Regression tests** fossilize a previously broken combination until the
  underlying rule is understood and moved into a law or example.

## Layout

- Private helper and invariant tests live beside the production code under
  `#[cfg(test)] mod tests` when they need private implementation access.
- Public crate behavior lives in integration tests under
  `crates/<crate>/tests/`.
- Shared integration-test harness code lives in `tests/support/` once two or
  more files need it. Keep it typed and explicit; do not hide behavior in
  stringly setup.
- Cross-cutting guardrails live in contract tests named for the promise they
  defend, for example `crate_boundary_contract.rs`.
- Property tests are named for laws, not implementation details, for example
  `property_laws.rs`.

## Current Suites

- `crates/leaven-engine/tests/engine_contract.rs::graph_surface`: scenario and example coverage
  for proposal application, lineage, sibling/candidate-tree views,
  informed-by edges, failed apply attempts, and recent failure refs through
  `RunContext`, plus an append-only graph law test.
- `crates/leaven-engine/tests/engine_contract.rs::context_services`: proposal/evaluation
  context scenarios for budget charging, error recording, cache policy,
  evidence references, durable evaluation-request recording on failed
  evaluations, proposal/evaluation/render context views, read-scoped
  assessment queries, and graph-backed evidence lookup.
- `crates/leaven-engine/tests/engine_contract.rs::budget_laws`: budget axis and sub-stage
  charging laws, including typed refusal of invalid seconds amounts.
- `crates/leaven-kernel/tests/kernel_contract.rs`: consolidated kernel
  contract harness covering property and regression coverage
  for finite, non-negative `Amount` construction, serde round trips, and
  saturating cost combination.
- `crates/leaven-engine/tests/engine_contract.rs::case_set_resolution`: evaluation-set
  resolution examples for partitions, explicit cases, set combinators,
  deterministic sampling, and typed refusal for tag-index-dependent sets.
- `crates/leaven-engine/tests/engine_contract.rs::trust_policy`: read-scope and actor-policy
  examples for hidden partitions and typed trust refusals.
- `crates/leaven-engine/tests/engine_contract.rs::engine_loop`: engine run-loop scenarios for
  continuation, optimizer completion, trust-policy wiring, callback dispatch,
  max-iteration refusal, and optimizer error closeout.
- `crates/leaven-engine/tests/engine_contract.rs::stage_trait_contracts`: static-to-dynamic
  stage adapter contracts for proposer, evaluator, preference, and stopper
  traits.
- `crates/leaven-engine/tests/engine_contract.rs::population_event_contract`: public event
  contract proving population reweighting uses finite weights and population
  observer defaults emit no events.
- `crates/leaven-engine/tests/engine_contract.rs::materializer_contract`: P4 materializer
  contracts for deterministic workspace writes, proposer-scoped read views,
  explicit cleanup, absence of old `WorkspaceRenderer` names, and
  `Create + None + informed_by(history)` lineage.
- `crates/leaven-surface/tests/part_contract.rs`: public surface example
  coverage proving `Part` carries identity, address, and typed view semantics
  without a framework-wide kind taxonomy.
- `crates/leaven/tests/scalar_keep_best.rs`: P1 end-to-end scenario from seed
  proposal through evaluation, inline evidence storage, keep-best population,
  callbacks, and best-candidate result.
- `crates/leaven/tests/pairwise_tournament.rs`: P2 end-to-end scenario for a
  registered pairwise judge, pairwise evidence storage by reference, fitted
  tournament population update, and best-candidate result.
- `crates/leaven/tests/gepa_parity.rs`: P3 end-to-end scenario for explicit
  edit-surface GEPA, per-case train evidence, train-filtered Pareto frontier
  updates, surface-edit lowering, and best-candidate result.
- `examples/p4_meta_harness_lite` via `just milestone-p4`: P4 runnable
  scenario for materialized workspace history, proposer-authored fresh
  artifacts, explicit cleanup, evidence refs, hidden test filtering,
  evaluator workspace isolation, and population update.
- `crates/leaven/tests/topology_contract.rs`: guardrails for the active
  workspace member list, crate/bin entrypoints, Leaven-to-Leaven dependency
  DAG, and cold-core leak boundaries.
- `crates/leaven-agentic-skill/tests/skill_agentic.rs`: skill-bank
  materialization/readback, workspace proposal parsing, patch-plan validation,
  atomic patch application, change reporting, and rollback evidence for failed
  atomic application.
- `crates/leaven-evidence/tests/evidence_contract.rs`,
  `crates/leaven-preference/tests/scalar.rs`,
  `crates/leaven-population/tests/population_contract.rs`, and
  `crates/leaven-store-inline/tests/evidence.rs`: finite scalar,
  command/trajectory, analyst fan-out, patch merge-tree, attribution evidence,
  scalar preference, keep-best, and inline store behavior now covered by the
  canonical coverage gate.
- `crates/leaven-population/tests/population_contract.rs` and
  `crates/leaven-gepa/tests/gepa_contract.rs`: P3 casewise Pareto frontier laws,
  partition filtering, GEPA surface ownership, surface-edit lowering, candidate
  selector separation, and proposer read-scope coverage.
- `crates/leaven-core/tests/proposal_contract.rs`: cold proposal constructors,
  causal lineage, informational references, clone behavior, and batch semantics.
- `crates/leaven-kernel/tests/kernel_contract.rs`: finite signed floats,
  amount/cost conversions, metered mapping, fingerprint ordering, durable error
  records, metadata ordering, typed IDs, and stage attribution display.
- `crates/leaven-workspace/tests/workspace_path.rs`: workspace path examples
  and property laws proving public paths remain relative, UTF-8, traversal-free,
  and explicit about the root path.
- `crates/leaven-workspace/tests/workspace_view.rs` and
  `crates/leaven-workspace-local/tests/local_workspace.rs`: P4 workspace
  substrate coverage for scoped writes/reads, unattached command refusal,
  backend local-mount semantics, factory allocation, unique local roots, and
  cleanup removal plus already-removed cleanup tolerance.
- `examples/trace2skill_spreadsheetbench/tests/manifest.rs`: mechanics-smoke
  for the Trace2Skill SpreadsheetBench-Verified 400-row release. It proves
  local upstream JSON rows lower into `leaven-eval` cases with source-row
  metadata and the paper's `0..200` train / `200..400` held-out split. It is
  not proof of spreadsheet execution, trajectory generation, skill evolution,
  or metric reproduction.
- `examples/trace2skill_spreadsheetbench/tests/run_artifacts.rs`:
  mechanics-smoke for importing upstream-shaped Trace2Skill run artifacts into
  Leaven evidence. It proves `results.json`, chat logs, and optional analysis
  reports lower into `AgentTrajectoryCorpusEvidence` for the training/evolving
  split, and that those imported trajectories can seed a pending Stage 2
  `AgentAnalystFanoutEvidence` manifest. It is not proof that those artifacts
  were generated by a live Qwen/vLLM SpreadsheetBench run or that analyst calls
  executed.
- `examples/trace2skill_spreadsheetbench/tests/patch_bridge.rs`:
  mechanics-smoke for lowering upstream-shaped fenced JSON patches into
  `SkillParsedPatchDocument`, then into `SkillPatchPlan` plus concrete
  `SkillBankChange` values and applying them through `SkillPatchApplication`.
  It proves patch parsing and application wiring only; it is not proof that
  analyst calls, hierarchical merge execution, prevalence policy, or metric
  reproduction ran.
- `examples/trace2skill_spreadsheetbench/tests/patch_replay.rs`:
  mechanics-smoke for replaying saved/live upstream JSON patch merge artifacts
  through `SkillPatchMergeTree` and `SkillPatchApplication`. It proves the
  artifact replay seam, evolved `SkillBank` output, and strict loader for
  upstream-shaped `--save-intermediates` JSON directories (`map_patches`,
  numeric `merge_level_N`, `final_patch.json`, optional
  `translated_final_patch.json`) only; it is not proof that the model-backed
  analysts or merge scheduler ran.
- `examples/memento_skills_read_write/scripts/run_tiny_live.sh --preflight`:
  no-spend sanity check for the Memento-Skills tiny Read-Write harness. It
  writes a proof contract under `tmp/memento_skills_read_write/` but does not
  execute Codex, train the router, or prove GAIA/HLE parity.
- `examples/skillreducer_tiny/scripts/run_tiny_live.sh --preflight`:
  no-spend sanity check for the SkillReducer tiny debloating harness. It writes
  a proof contract under `tmp/skillreducer_tiny/` but does not execute Codex,
  search a full skill corpus, or prove SkillsBench parity.
- `examples/d2skill_tiny/scripts/run_tiny_live.sh --preflight`: no-spend
  sanity check for the D2Skill paired-rollout skill-bank harness. It writes a
  proof contract under `tmp/d2skill_tiny/` but does not execute Codex, train
  GRPO, or prove ALFWorld/WebShop parity.
- `examples/trace2skill_tiny_live/scripts/run_tiny_live.sh --preflight`:
  no-spend sanity check for the Trace2Skill tiny live trajectory-to-skill
  harness. It writes a proof contract under `tmp/trace2skill_tiny_live/` but
  does not execute Codex, run SpreadsheetBench, or prove Qwen/vLLM parity.

## Review Rules

- A test without a claim is noise.
- A test that only passes through ceremony belongs at a lower layer or should be
  deleted.
- Public holes for tests are forbidden. Use public behavior, crate-private unit
  tests, or a typed test harness.
- Process environment mutation must be isolated behind a helper that restores
  state. Prefer explicit typed setup.
