# P3 GEPA Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fully satisfy P3 GEPA parity over an explicit edit surface: GEPA remains one optimizer composition over `EditSurface`, casewise evidence feeds a casewise Pareto frontier without aggregate collapse, train/validation visibility is enforced, and the runnable milestone plus focused tests prove the public contract.

**Architecture:** Keep per-case evidence in `leaven-evidence`, casewise Pareto population state in `leaven-population`, edit-surface and part vocabulary in `leaven-surface`, GEPA strategy values in `leaven-gepa`, runtime mutation/evaluation/trust in `leaven-engine`, and the tiny part-map artifact/evaluator in the P3 example or umbrella integration test. `leaven-core` stays cold and does not learn about surfaces, GEPA policy, graph storage, stores, renderers, or engine runtime.

**Tech Stack:** Rust 2024, Leaven workspace crates, `leaven-store-inline` for deterministic evidence storage, `futures::executor::block_on` for runnable examples and scenario tests, `proptest` for frontier/surface laws where a generated input space matters, focused `cargo nextest` gates, `just milestone-p3`, and final `just check`.

---

## Governing Requirements

- `AGENTS.md`
- `docs/specs/milestone_examples_behavioral_contract.md`, P3 section
- `docs/specs/initial_library.md`, especially Prototype 3 and GEPA sections
- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`, `leaven-gepa` topology
- `docs/specs/guiding_principles.md`
- `docs/testing/README.md`
- `docs/philosophy/skills/leaven-type-design/SKILL.md`
- `docs/philosophy/skills/leaven-trait-design/SKILL.md`
- `docs/philosophy/skills/leaven-error-design/SKILL.md`
- `docs/philosophy/skills/leaven-test-design/SKILL.md`

## Current Baseline

The live tree already has the main P3 surfaces:

- `crates/leaven-evidence/src/casewise.rs` defines sparse `CasewiseEvidence` with deterministic duplicate canonicalization.
- `crates/leaven-population/src/pareto_frontier.rs` defines `ParetoFrontier`, `ParetoFrontierBuilder`, `PartitionFilter`, and casewise scalar observation.
- `crates/leaven-gepa/src/{optimizer,selector,part_selector,proposer,gate,validation}.rs` defines `Gepa`, `CandidateSelector`, `PartSelector`, deterministic `ReflectiveMutation`, `RoundRobinPart`, `ParetoFrequencyWeighted`, and gate policies.
- `examples/p3_gepa_parity` already runs one deterministic GEPA-like step through `RunContext`, `PartMapSurface`, `ReflectiveMutation`, `CasewiseEvidence`, and `ParetoFrontier`.
- Baseline commands already pass:
  - `just milestone-p3`
  - `cargo nextest run -p leaven-evidence --test casewise`
  - `cargo nextest run -p leaven-population --test pareto_frontier`
  - `cargo nextest run -p leaven-gepa --test gepa_smoke`

The remaining work is to close the still-fuzzy behavioral bullets: make partition filters operational, add stronger property/visibility coverage, make the example match the spec's local edit type, and add an umbrella end-to-end test.

## Non-Negotiables

- No compatibility paths, aliases, or duplicate old/new GEPA lanes.
- No GEPA-specific concepts in `leaven-core`, `leaven-engine`, or `Artifact`.
- No behavior in `lib.rs`; only module declarations and curated re-exports.
- `RunContext` remains the only public graph mutation path.
- GEPA owns `S: EditSurface<P::Artifact>` and lowers surface edits through `S::change_part(...)` before recording `ProposalEffect::Change`.
- `Population` and `CandidateSelector` stay separate values.
- P3 uses deterministic fake reflection only; no LLM/provider calls, network, or concrete workspace backend.
- Per-case evidence is handed to `ParetoFrontier` before any aggregate score is used for the gate.
- Validation/test partitions are hidden from proposer-visible graph views and are not used by reflective mutation.
- Tests assert public/capability behavior and kill plausible wrong implementations.

## Spec-To-Code Map

| Requirement | Owner | Current status | Work |
| --- | --- | --- | --- |
| Sparse `CasewiseEvidence` with `new`, `outcomes`, `get`, and deterministic duplicate policy | `leaven-evidence` | Implemented and tested | Keep; no code change unless tests expose a gap |
| Casewise Pareto strict dominance, non-regressing improvement, incomparable candidates, stable best | `leaven-population` | Implemented and mostly tested | Add generated order-independence coverage |
| Partition filters exclude observations before frontier update | `leaven-population` | Filter is stored but not operational | Add a partition-aware observation method and tests |
| GEPA module shape and strategy slots | `leaven-gepa` | Implemented as real modules | Keep `lib.rs` as map only |
| GEPA owns surface and lowers surface edit to artifact-native change | `leaven-gepa` | Implemented and smoke tested | Strengthen surface lowering property |
| Engine knows nothing about part selectors, GEPA gates, reflective mutation, or Pareto-frequency weighting | topology contract plus crate layout | Satisfied | Avoid new engine hooks |
| P3 example uses `PartMapArtifact`, `PartMapSurface`, `PartMapEdit` | example crate | Uses `String` edit directly | Introduce local `PartMapEdit::Replace` |
| `AssessmentGranularity::PerCase` requested | example and tests | Implemented | Assert in e2e test |
| Per-case evidence reaches frontier before aggregate gate | example and tests | Implemented in order but not externally tested | Add umbrella scenario assertions |
| Surface fingerprint participates in cache identity where surface-derived evaluation/rendering is cached | `leaven-surface` docs / future surface-derived cache users | No cached surface-derived P3 path exists | Leave as a documented invariant; do not invent a cache hook in P3 |
| Validation/test partitions hidden from proposer views | `leaven-engine` trust plus P3 tests | Engine has generic tests | Add P3/GEPA-focused smoke test using proposer read scope |
| `PartMapSurface::change_part` is pure | example/test surface | Implemented | Assert original artifact unchanged in example/e2e |

## Task 1: Make Pareto Partition Filtering Operational

**Files:**
- Modify: `crates/leaven-population/src/pareto_frontier.rs`
- Modify: `crates/leaven-population/tests/pareto_frontier.rs`

**Claim:** A frontier with `PartitionFilter::Only({TRAIN})` ignores observations from `VALIDATION` before mutating scores/frontier membership, and admits matching `TRAIN` observations normally.

**Implementation Shape:**
- Keep the existing minimum API `observe_casewise_scalar(...)` for unpartitioned/all-filter callers.
- Add `observe_partitioned_casewise_scalar(&mut self, partition: &PartitionId, candidate, assessment, evidence)`.
- Route both public methods through a private observation helper.
- For `PartitionFilter::Only`, reject `None` or non-member partitions before touching `scores` or recomputing the frontier.
- Return `PopulationEvent::Ignored` for filtered observations with an explicit reason.

**Verification:**
- `cargo nextest run -p leaven-population --test pareto_frontier`

## Task 2: Add Frontier And Surface Property Coverage

**Files:**
- Modify: `crates/leaven-population/tests/pareto_frontier.rs`
- Modify: `crates/leaven-gepa/Cargo.toml`
- Modify: `crates/leaven-gepa/tests/gepa_smoke.rs`

**Claims:**
- Frontier membership and best candidate are independent of observation order for the same generated candidate/evidence set.
- Surface lowering followed by artifact apply changes only the selected part over generated two-part maps.

**Implementation Shape:**
- Use `proptest` in `leaven-population` for generated finite scores across two candidates and two cases.
- Add `proptest` as a dev-dependency of `leaven-gepa`.
- Use `proptest` in `leaven-gepa/tests/gepa_smoke.rs` for generated selected part, selected edit, and untouched part value.

**Verification:**
- `cargo nextest run -p leaven-population --test pareto_frontier`
- `cargo nextest run -p leaven-gepa --test gepa_smoke`

## Task 3: Add P3 Proposer-Visibility Coverage

**Files:**
- Modify: `crates/leaven-gepa/tests/gepa_smoke.rs`

**Claim:** Hidden validation partitions do not appear in proposer-visible graph views used by GEPA-style reflective mutation.

**Implementation Shape:**
- Build a small `RunGraph` with one candidate and one assessment over `EvaluationSet::Partition(VALIDATION)`.
- Create a `RunContext` with `TrustPolicy::hide_from_proposers([VALIDATION])`.
- Call a local inspecting proposer through `ctx.propose(...)`.
- In the proposer, assert `ctx.read_scope()` contains the hidden partition and `ctx.graph().assessment(validation_assessment)` returns `None`.

**Verification:**
- `cargo nextest run -p leaven-gepa --test gepa_smoke hidden_validation_partitions_are_not_visible_to_gepa_proposers`

## Task 4: Harden The P3 Example

**Files:**
- Modify: `examples/p3_gepa_parity/src/main.rs`

**Claims:**
- The example matches the spec's local edit vocabulary with `PartMapEdit::Replace(String)`.
- The frontier observes only train-partition casewise evidence through the partition-aware method.
- The reflective mutation leaves the validation partition unused and the non-selected `search` part unchanged.

**Implementation Shape:**
- Add `enum PartMapEdit { Replace(String) }`.
- Change `PartMapSurface::Edit` and `ReflectiveMutation` usage to use `PartMapEdit`.
- Build `ParetoFrontier` with a train-only partition filter.
- Use `observe_partitioned_casewise_scalar(&PartitionId::from(TRAIN), ...)` for both baseline and candidate observations.

**Verification:**
- `just milestone-p3`

## Task 5: Add P3 End-To-End Umbrella Test

**Files:**
- Create: `crates/leaven/tests/gepa_parity.rs`

**Claim:** The umbrella public surface can run P3 end to end: seed multi-part artifact, select a candidate/part via GEPA, lower a surface edit into `ProposalEffect::Change`, apply through `RunContext`, request `PerCase` evaluation over train cases, store casewise evidence by reference, update `ParetoFrontier`, and return an improved best candidate while validation stays hidden.

**Implementation Shape:**
- Mirror `examples/p3_gepa_parity/src/main.rs` as a scenario test, keeping fixtures local to the test.
- Assert:
  - best artifact has `answer == "improved answer"` and unchanged `search`,
  - two independent per-case evaluation requests were recorded,
  - both recorded requests target `EvaluationSet::Partition(TRAIN)`,
  - two assessments were recorded and evidence lookup returns casewise train cases,
  - the candidate proposal effect is `ProposalEffect::Change`,
  - `PopulationUpdated` and `OptimizationEnded` appear in the event stream,
  - proposer-visible read scope hides `VALIDATION` through the GEPA smoke test from Task 3.

**Verification:**
- `cargo nextest run -p leaven --test gepa_parity`

## Task 6: Update Testing Documentation

**Files:**
- Modify: `docs/testing/README.md`

**Claim:** The durable test contract names the P3 suites and direct proof path.

**Implementation Shape:**
- Add `crates/leaven/tests/gepa_parity.rs`.
- Add `crates/leaven-evidence/tests/casewise.rs`, `crates/leaven-population/tests/pareto_frontier.rs`, and `crates/leaven-gepa/tests/gepa_smoke.rs` to the current suite list if absent.

**Verification:**
- Check referenced paths exist.

## Task 7: Final Verification

**Commands:**

```bash
just milestone-p3
cargo nextest run -p leaven-evidence --test casewise
cargo nextest run -p leaven-population --test pareto_frontier
cargo nextest run -p leaven-gepa --test gepa_smoke
cargo nextest run -p leaven --test gepa_parity
just test
just check
```

**Completion Criteria:**
- All commands pass.
- P3 success criteria from the milestone spec are directly tested or explicitly bounded when no surface-derived cached evaluation/rendering path exists.
- No coverage floor is lowered.
- No `lib.rs` gains behavior.
- `leaven-core` remains cold and engine graph mutation remains private to `RunContext`.
- `jj st` shows only intentional P3 plan/test/code/doc changes.
