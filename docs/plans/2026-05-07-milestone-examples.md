# Milestone Examples Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the placeholder milestone crates under `examples/` with runnable examples that prove P0 through P4 against the v0.2.2 spec, filling only the library behavior each milestone genuinely needs.

**Architecture:** Treat each example crate as an executable acceptance test for one milestone. Library behavior lands in the crate that owns the relevant knowledge boundary: cold algebra in `leaven-core`, graph/context/runtime behavior in `leaven-engine`, evidence shapes in `leaven-evidence`, standard populations in `leaven-population`, standard preferences in `leaven-preference`, workspace substrate in `leaven-workspace`, and GEPA-specific rhythm in `leaven-gepa`. Use hard cutovers only; do not add compatibility aliases such as `WorkspaceRenderer` once `Materializer` lands.

**Tech Stack:** Rust 2024, existing Leaven workspace crates, `futures::executor::block_on` for synchronous examples over async stage traits, `leaven-store-inline` for deterministic evidence storage, `cargo run -p <example-crate>` milestone gates, and final `just check`.

---

## Governing Requirements

- `docs/specs/milestone_examples_behavioral_contract.md`
- `docs/specs/initial_library.md`
- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`
- `docs/specs/guiding_principles.md`

## Original Baseline

- `cargo check --workspace` passed on the working tree when this plan was
  written.
- `cargo check --workspace --examples` is a no-op because the milestone examples are workspace packages, not Cargo example targets.
- `examples/p0_graph_skeleton`, `examples/p1_keep_best`, `examples/p2_pairwise_tournament`, `examples/p3_gepa_parity`, and `examples/p4_meta_harness_lite` compile but each `main.rs` is only a type-name placeholder.
- P1 behavior already exists as a real integration proof in `crates/leaven/tests/scalar_keep_best.rs`.
- P2 through P4 still need real library behavior: pairwise evidence/tournament state, GEPA rhythm over an edit surface, and materializer/workspace/agentic-create flow.
- The plan started from a codebase that still exported `WorkspaceRenderer`; the
  v0.2.2 spec requires `Materializer` with no compatibility alias.

## Non-Negotiables

- Keep `RunContext` as the only public mutation path into `RunGraph`.
- Do not make graph storage public to make examples easier.
- Keep `leaven-core` cold: no graph, engine, store, workspace, renderer, GEPA, or adapter knowledge.
- Examples may define tiny local artifacts/proposers/evaluators when they are demonstration fixtures, but reusable evidence, preference, population, workspace, and GEPA behavior belongs in the owning library crates.
- Each milestone must have a direct command that runs the example, not only a compile check.
- Do not lower coverage floors or add source-path coverage ignore regexes to
  land behavior.

## Task 1: Add The Milestone Gate

**Files:**
- Modify: `Justfile`
- Modify: `docs/testing/README.md`
- Modify: `docs/plans/2026-05-07-milestone-examples.md` if command names change during execution

**Steps:**
1. Add a `just milestone-examples` target that runs:
   - `cargo run -p p0_graph_skeleton`
   - `cargo run -p p1_keep_best`
   - `cargo run -p p2_pairwise_tournament`
   - `cargo run -p p3_gepa_parity`
   - `cargo run -p p4_meta_harness_lite`
2. Add narrower targets while iterating:
   - `just milestone-p0`
   - `just milestone-p1`
   - `just milestone-p2`
   - `just milestone-p3`
   - `just milestone-p4`
3. Document that `cargo check --workspace --examples` is not the right proof command for this repo layout.
4. Keep `just check` as the completion gate.

**Verification:**
- Run `just milestone-p0` after P0 lands.
- Expand the all-milestone command only when each milestone stops being a placeholder.

## Task 2: Make P0 A Real Graph Skeleton

**Files:**
- Modify: `examples/p0_graph_skeleton/Cargo.toml`
- Modify: `examples/p0_graph_skeleton/src/main.rs`

**Implementation Shape:**
- Define local `TextArtifact`, `TextChange`, and `TextError`.
- Implement `leaven::Artifact` for `TextArtifact`.
- Construct `leaven::engine::RunGraph` plus `leaven::engine::BudgetLedger`.
- Use `leaven::RunContext` to:
  - insert a seed,
  - record a `Proposal::create(...)`,
  - record a `Proposal::mutate(...)`,
  - apply both proposals,
  - assert parent/child lineage through `RunGraphView`.
- Print a compact final summary: seed id, created candidate id, mutated candidate id, and event count.

**Do Not:**
- Do not expose graph storage maps.
- Do not add a helper crate for one tiny example.

**Verification:**
- Run `cargo run -p p0_graph_skeleton`.
- Run `cargo nextest run -p leaven-engine --test graph_surface`.

## Task 3: Promote The Existing P1 Proof Into `examples/p1_keep_best`

**Files:**
- Modify: `examples/p1_keep_best/Cargo.toml`
- Modify: `examples/p1_keep_best/src/main.rs`
- Inspect: `crates/leaven/tests/scalar_keep_best.rs`

**Implementation Shape:**
- Copy the proven P1 flow from `crates/leaven/tests/scalar_keep_best.rs` into the example, trimming test-only callback assertions into runtime assertions.
- Add dependencies needed by the example package:
  - `futures`
  - `leaven-store-inline`
- Keep `TextArtifact`, `TwoMutations`, `TextLengthEvaluator`, and `ScalarKeepBestOptimizer` local to the example until a second example needs them.
- The example must prove:
  - seed artifact is inserted,
  - proposer emits two alternatives,
  - `RunContext` applies both proposals,
  - evaluator stores `ScalarEvidence`,
  - `KeepBest` observes assessments,
  - the best artifact is `"aaa"`.

**Verification:**
- Run `cargo run -p p1_keep_best`.
- Run `cargo nextest run -p leaven --test scalar_keep_best`.

## Task 4: Implement Pairwise Evidence For P2

**Files:**
- Modify: `crates/leaven-evidence/src/lib.rs`
- Create: `crates/leaven-evidence/src/pairwise.rs`
- Create or modify: `crates/leaven-evidence/tests/pairwise.rs`

**Implementation Shape:**
- Replace the inline skeleton `pairwise` module with a real module file.
- Define:
  - `PairwiseJudgment::{Left, Right, Tie}`
  - `PairwiseJudgmentEvidence { judgment, confidence, rationale }`
- Use `FiniteF64` for optional confidence so non-finite confidence cannot enter evidence.
- Implement `leaven_core::Evidence` for `PairwiseJudgmentEvidence`.

**Verification:**
- Run `cargo nextest run -p leaven-evidence --test pairwise`.

## Task 5: Implement Tournament Population State

**Files:**
- Modify: `crates/leaven-population/src/lib.rs`
- Create: `crates/leaven-population/src/tournament.rs`
- Create: `crates/leaven-population/tests/tournament.rs`

**Implementation Shape:**
- Move `BradleyTerryFit` and `TournamentPopulation` out of skeleton declarations into real code.
- Implement `BradleyTerryFit` as real fitted state, not a renamed win counter. A small deterministic online logistic update is enough for P2.
- Implement `TournamentPopulation::observe_pairwise(left, right, assessment_id, evidence)` returning `PopulationEvent`s.
- Implement `TournamentPopulation::best()` from the fitted ability scores.
- Keep the current direct-observation shape explicit. Do not pretend `Population::observe_assessment(assessment_id, graph)` can read evidence; graph stores evidence refs, not evidence values.

**Verification:**
- Run `cargo nextest run -p leaven-population --test tournament`.

## Task 6: Add Evaluator Registry Dispatch

**Files:**
- Modify: `crates/leaven-kernel/src/ids.rs`
- Modify: `crates/leaven-engine/src/stage/evaluator.rs`
- Modify: `crates/leaven-engine/src/engine.rs`
- Modify: `crates/leaven-engine/src/context/run_context.rs`
- Create or modify: `crates/leaven-engine/tests/evaluator_registry.rs`

**Implementation Shape:**
- Add `EvaluatorId::PAIRWISE_JUDGE`.
- Extend `DynEvaluator` so erased evaluators expose `fingerprint()` and `cache_policy(...)`; otherwise registry dispatch cannot preserve the current cache contract.
- Add an engine-owned evaluator registry and `EngineBuilder::evaluator(...)`.
- Add `RunContext::evaluate(evaluator_id, request)` as registry dispatch over the same recording, cache, evidence-store, budget, and event path as `evaluate_with`.
- Keep `evaluate_with` as the direct static path; it is not a compatibility shim, it is the lower-friction path for stage-owned evaluators.

**Verification:**
- Run `cargo nextest run -p leaven-engine --test evaluator_registry`.
- Run existing context tests: `cargo nextest run -p leaven-engine --test context_services`.

## Task 7: Make P2 Pairwise Tournament Runnable

**Files:**
- Modify: `examples/p2_pairwise_tournament/Cargo.toml`
- Modify: `examples/p2_pairwise_tournament/src/main.rs`

**Implementation Shape:**
- Define a local text artifact and deterministic pairwise judge evaluator.
- Register the judge under `EvaluatorId::PAIRWISE_JUDGE`.
- Use `EvaluationRequest::Pairwise` with `PairOrder::Ordered`.
- Store `PairwiseJudgmentEvidence` in `InlineEvidenceStore`.
- Let `TournamentPopulation` observe the pairwise assessment and pick the winning candidate.
- Print winner, judgment, and fitted score summary.

**Verification:**
- Run `cargo run -p p2_pairwise_tournament`.
- Run `just milestone-p2` after the Justfile target exists.

## Task 8: Add Casewise Evidence And Minimal Pareto Frontier For P3

**Files:**
- Modify: `crates/leaven-evidence/src/lib.rs`
- Create: `crates/leaven-evidence/src/casewise.rs`
- Modify: `crates/leaven-population/src/lib.rs`
- Create or modify: `crates/leaven-population/src/pareto.rs`
- Create: `crates/leaven-population/tests/pareto_frontier.rs`

**Implementation Shape:**
- Add `CasewiseEvidence<E>` for per-case measurement where `E: Evidence`.
- Add a minimal `ParetoFrontier` that can admit candidates based on case-keyed scalar evidence.
- Add a `partition_filter` builder method only if the example uses train/validation split; otherwise keep it planned for P4/GEPA expansion.
- Keep Pareto state in `leaven-population`, not `leaven-engine`.

**Verification:**
- Run `cargo nextest run -p leaven-evidence --test casewise`.
- Run `cargo nextest run -p leaven-population --test pareto_frontier`.

## Task 9: Implement The Minimal GEPA Rhythm

**Files:**
- Modify: `crates/leaven-gepa/src/lib.rs`
- Create: `crates/leaven-gepa/src/selector.rs`
- Create: `crates/leaven-gepa/src/part_selector.rs`
- Create: `crates/leaven-gepa/src/proposer.rs`
- Create: `crates/leaven-gepa/src/optimizer.rs`
- Create: `crates/leaven-gepa/tests/gepa_smoke.rs`

**Implementation Shape:**
- Replace skeleton unit structs with real, minimal policy values:
  - `CandidateSelector`
  - `RoundRobinPart`
  - `ReflectiveMutation`
  - `StrictImprovement`
  - `Gepa`
- Make `Gepa<P, S, Pop>` own `S: EditSurface<P::Artifact>`.
- Let GEPA proposers emit surface edits, then let GEPA lower them through `S::change_part(...)` into `ProposalEffect::Change`.
- Keep LLM calls out of P3. The reflective proposer can be deterministic; P3 proves the Leaven shape, not provider quality.

**Verification:**
- Run `cargo nextest run -p leaven-gepa --test gepa_smoke`.

## Task 10: Make P3 GEPA Parity Runnable

**Files:**
- Modify: `examples/p3_gepa_parity/Cargo.toml`
- Modify: `examples/p3_gepa_parity/src/main.rs`

**Implementation Shape:**
- Define local `PartMapArtifact` and `PartMapSurface`.
- Run a deterministic GEPA step over two or three parts.
- Request `AssessmentGranularity::PerCase`.
- Use casewise scalar evidence and the Pareto frontier.
- Assert that the chosen candidate improves at least one case without regressing the protected validation case.

**Verification:**
- Run `cargo run -p p3_gepa_parity`.
- Run `just milestone-p3`.

## Task 11: Hard-Cut Workspace Rendering To Materialization

**Files:**
- Modify: `crates/leaven-engine/src/stage/renderer.rs`
- Modify: `crates/leaven-engine/src/lib.rs`
- Modify: `crates/leaven-engine/src/context/render_context.rs`
- Modify: `crates/leaven-render/src/lib.rs`
- Modify: `crates/leaven/src/lib.rs`
- Modify: `crates/leaven/src/prelude.rs`
- Modify: `crates/leaven-workspace/src/lib.rs`
- Modify: `crates/leaven-workspace/src/workspace.rs`
- Modify: `crates/leaven-workspace/src/view.rs`
- Create or modify: `crates/leaven-engine/tests/materializer_contract.rs`
- Create or modify: `crates/leaven-workspace/tests/workspace_path.rs`

**Implementation Shape:**
- Rename `WorkspaceRenderer` to `Materializer`; remove the old name entirely.
- Rename workspace-renderer structs in `leaven-render` to materializer names.
- Introduce `WorkspacePath` and use it in public workspace file APIs.
- Keep host `PathBuf` behind local backend internals. Public examples should not need `local_mount()`.
- Add a minimal local workspace backend if the current local crate is still only a skeleton.

**Verification:**
- Run `cargo nextest run -p leaven-engine --test materializer_contract`.
- Run `cargo nextest run -p leaven-workspace --test workspace_path`.
- Run `cargo check --workspace` to catch all hard-cut rename fallout.

## Task 12: Make P4 Meta-Harness Lite Runnable

**Files:**
- Modify: `examples/p4_meta_harness_lite/Cargo.toml`
- Modify: `examples/p4_meta_harness_lite/src/main.rs`
- Modify: `crates/leaven-evidence/src/lib.rs` only if a reusable command or agent trajectory evidence shape is needed

**Implementation Shape:**
- Use a local workspace factory.
- Materialize seed artifact, selected run history, and recent evidence into backend-neutral workspace paths.
- Use a deterministic fake agent proposer that reads the materialized workspace and returns `ProposalEffect::Create`.
- Evaluate the created harness with a deterministic repo-task evaluator.
- Store command/trajectory evidence by reference through the evidence store.
- Assert the run completes and the workspace cleanup path runs explicitly.

**Verification:**
- Run `cargo run -p p4_meta_harness_lite`.
- Run `just milestone-p4`.

## Task 13: Final Documentation And Gate

**Files:**
- Modify: `docs/testing/README.md`
- Modify: `docs/specs/p1_scalar_keep_best_verification_contract.md` only if it needs status or deferral updates
- Modify: `Justfile`

**Steps:**
1. Update testing docs with:
   - milestone example commands,
   - which crates carry behavior now,
   - which coverage exclusions were narrowed.
2. Check that every behavior-bearing crate added in this plan has tests.
3. Run:
   - `just milestone-examples`
   - `just test`
   - `just check`
4. If `just check` fails only on coverage, add tests or narrow exclusions correctly. Do not lower floors.

**Completion Definition:**
- All five milestone example binaries run real flows.
- P0/P1 use existing core/engine behavior without new abstraction.
- P2 proves pairwise evidence and tournament population.
- P3 proves GEPA as one optimizer over an explicit edit surface.
- P4 proves materialization, backend-neutral workspace paths, fresh authoring, and explicit cleanup.
- `just check` passes.
