# Leaven Testing Contract

Leaven uses tests to collapse the remaining implementation space after the
type, trait, and error surfaces are honest. Every test must name a real claim
and kill a plausible wrong implementation.

## Canonical Check

Run the full local gate before claiming behavior is complete:

```bash
just check
```

`just check` runs formatting, the production line-count lint, clippy with
workspace library/tool targets, the nextest workspace suite, doctests, and the
line/branch coverage summary. The default gate excludes milestone example
packages; use the explicit milestone recipes when an example workflow is the
claim under test. Use narrower recipes only while iterating:

```bash
just lint
just test
just test-one <nextest selector>
just test-stress 20 <nextest selector>
just coverage
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

The canonical full test suite has a hard wall-clock SLA:

```text
just test must finish in <30s
```

`just test` enforces this directly. The SLA covers the nextest workspace suite
and workspace doctests for library/tool packages that contain Rust doctest
fences. Milestone examples stay out of the default SLA and run through explicit
`just milestone-*` recipes. Empty doctest harnesses are skipped because they
prove no examples while adding process-startup cost. If the suite crosses the
line, do not add a second slow lane; reduce fixture cost, property-test case
count, setup work, doctest harness fan-out, or assertion altitude until the
default suite is back under the SLA.

Coverage is a ratchet. Raise `coverage_line_floor` and
`coverage_branch_floor` in the root `Justfile` when coverage improves; do not
lower either floor to land weaker tests.

The v0.2.1b topology cutover adds many spec-listed crate skeletons whose job is
to enforce dependency direction before their behavior lands. The coverage gate
keeps a `98.5` line floor and `85.8` branch floor over production/source
behavior. It runs the workspace tests, then runs every milestone binary and
`xtask` under `cargo llvm-cov run` before reporting. The enforced denominator
excludes test harness files and `#[cfg(test)] mod ...` blocks after execution,
so tests can exercise production code without becoming code that must itself
be covered. Line and branch coverage are both enforced from the lcov report so
generic monomorphizations do not create duplicate missed-line denominators.
Empty map crates and unimplemented skeleton crates naturally add no executable
denominator; once a crate gains runtime behavior, that behavior is part of the
canonical coverage surface and needs contract tests in the same change.
Coverage keeps the exercised surface honest; it does not promote proxy examples
or placeholder public names into mature product contracts.

Coverage's P5 run is not the same as `just milestone-p5`: the coverage script
runs the package with a generated `--run-dir` and without the live Codex gate.
Coverage's P8 run is the deterministic LM-backed path. Treat both as coverage
of executable code, not release proof for live-provider, durable cache, or live
AIME improvement claims.

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

- `crates/leaven-engine/tests/graph_surface.rs`: scenario and example coverage
  for proposal application, lineage, sibling/candidate-tree views,
  informed-by edges, failed apply attempts, and recent failure refs through
  `RunContext`, plus an append-only graph law test.
- `crates/leaven-engine/tests/context_services.rs`: proposal/evaluation
  context scenarios for budget charging, error recording, cache policy,
  evidence references, durable evaluation-request recording on failed
  evaluations, proposal/evaluation/render context views, read-scoped
  assessment queries, and graph-backed evidence lookup.
- `crates/leaven-engine/tests/budget_laws.rs`: budget axis and sub-stage
  charging laws, including typed refusal of invalid seconds amounts.
- `crates/leaven-kernel/tests/cost_amount.rs`: property and regression coverage
  for finite, non-negative `Amount` construction, serde round trips, and
  saturating cost combination.
- `crates/leaven-engine/tests/case_set_resolution.rs`: evaluation-set
  resolution examples for partitions, explicit cases, set combinators,
  deterministic sampling, and typed refusal for tag-index-dependent sets.
- `crates/leaven-engine/tests/trust_policy.rs`: read-scope and actor-policy
  examples for hidden partitions and typed trust refusals.
- `crates/leaven-engine/tests/engine_loop.rs`: engine run-loop scenarios for
  continuation, optimizer completion, trust-policy wiring, callback dispatch,
  max-iteration refusal, and optimizer error closeout.
- `crates/leaven-engine/tests/stage_trait_contracts.rs`: static-to-dynamic
  stage adapter contracts for proposer, evaluator, preference, and stopper
  traits.
- `crates/leaven-engine/tests/population_event_contract.rs`: public event
  contract proving population reweighting uses finite weights and population
  observer defaults emit no events.
- `crates/leaven-engine/tests/materializer_contract.rs`: P4 materializer
  contracts for deterministic workspace writes, proposer-scoped read views,
  explicit cleanup, absence of old `WorkspaceRenderer` names, and
  `Create + None + informed_by(history)` lineage.
- `crates/leaven-derive/tests/derive_macros.rs`: `trybuild` contract proving
  reserved derive macros fail explicitly until their real codegen contracts
  land.
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
- `crates/leaven/tests/topology_contract.rs`: guardrails for the full corrected
  v0.2.1b workspace member list, `src/lib.rs` skeleton presence,
  Leaven-to-Leaven dependency DAG, and cold-core leak boundaries.
- `crates/leaven-evidence/tests/scalar.rs`,
  `crates/leaven-evidence/tests/command.rs`,
  `crates/leaven-evidence/tests/casewise.rs`,
  `crates/leaven-evidence/tests/attribution.rs`,
  `crates/leaven-preference/tests/scalar.rs`,
  `crates/leaven-population/tests/keep_best.rs`, and
  `crates/leaven-store-inline/tests/evidence.rs`: finite scalar and
  attribution evidence, scalar preference, keep-best, and inline store behavior
  now covered by the canonical coverage gate.
- `crates/leaven-population/tests/pareto_frontier.rs` and
  `crates/leaven-gepa/tests/gepa_smoke.rs`: P3 casewise Pareto frontier laws,
  partition filtering, GEPA surface ownership, surface-edit lowering, candidate
  selector separation, and proposer read-scope coverage.
- `crates/leaven-core/tests/proposal_contract.rs`: cold proposal constructors,
  causal lineage, informational references, clone behavior, and batch semantics.
- `crates/leaven-kernel/tests/finite_f64.rs`,
  `crates/leaven-kernel/tests/cost_amount.rs`, and
  `crates/leaven-kernel/tests/identity_metadata.rs`: finite signed floats,
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

## Review Rules

- A test without a claim is noise.
- A test that only passes through ceremony belongs at a lower layer or should be
  deleted.
- Public holes for tests are forbidden. Use public behavior, crate-private unit
  tests, or a typed test harness.
- Process environment mutation must be isolated behind a helper that restores
  state. Prefer explicit typed setup.
