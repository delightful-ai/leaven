# P2 Pairwise Tournament Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fully satisfy the P2 pairwise tournament contract with a real pairwise evaluator path, fitted tournament population state, runnable milestone example, and tests that cover the behavioral and property claims in the spec.

**Architecture:** Keep pairwise measurement in `leaven-evidence`, fitted pairwise population state in `leaven-population`, evaluator registry and graph/runtime behavior in `leaven-engine`, and the tiny text artifact/judge fixture inside `examples/p2_pairwise_tournament` or integration tests. `leaven-core` only owns shape-neutral evaluation vocabulary; graph storage stays crate-private and all mutation stays behind `RunContext`.

**Tech Stack:** Rust 2024, Leaven workspace crates, `leaven-store-inline` for deterministic evidence storage, `futures::executor::block_on` for runnable examples, `proptest` for property tests, `cargo nextest` focused gates, `just milestone-p2`, and final `just check`.

---

## Governing Requirements

- `AGENTS.md`
- `docs/specs/milestone_examples_behavioral_contract.md`, P2 section
- `docs/specs/initial_library.md`
- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`
- `docs/specs/guiding_principles.md`
- `docs/testing/README.md`
- `docs/philosophy/skills/leaven-type-design/SKILL.md`
- `docs/philosophy/skills/leaven-trait-design/SKILL.md`
- `docs/philosophy/skills/leaven-error-design/SKILL.md`
- `docs/philosophy/skills/leaven-test-design/SKILL.md`

## Current Baseline

The live tree already has the main P2 surfaces:

- `crates/leaven-evidence/src/pairwise.rs` defines `PairwiseJudgment` and `PairwiseJudgmentEvidence`.
- `crates/leaven-population/src/tournament.rs` defines `BradleyTerryFit` and `TournamentPopulation`.
- `crates/leaven-kernel/src/ids.rs` defines `EvaluatorId::PAIRWISE_JUDGE`.
- `crates/leaven-engine` has evaluator registry dispatch through `EngineBuilder::evaluator(...)` and `RunContext::evaluate(...)`.
- `examples/p2_pairwise_tournament` runs and chooses the longer text candidate.

The remaining work is hardening against the exact P2 property bullets and making the test harness more explicit.

## Non-Negotiables

- No compatibility paths, aliases, or duplicate old/new evaluator lanes.
- No public graph map access; tests and examples go through `RunGraphView` and `RunContext`.
- No behavior in `lib.rs`; only module declarations and curated exports.
- `leaven-core` stays cold and does not learn about tournament state, evidence stores, engine runtime, or pairwise policy.
- Pairwise must stay pairwise: one `EvaluationRequest::Pairwise` yields one pairwise assessment over two candidates.
- Graph assessments store `EvidenceRef`; optimizers read evidence through the evidence store or receive it explicitly.
- Every test names a claim and kills a plausible wrong implementation.

## Task 1: Verify Current P2 Behavior

**Files:**
- Inspect: `examples/p2_pairwise_tournament/src/main.rs`
- Inspect: `crates/leaven-evidence/src/pairwise.rs`
- Inspect: `crates/leaven-population/src/tournament.rs`
- Inspect: `crates/leaven-engine/src/context/run_context.rs`
- Inspect: `crates/leaven-engine/tests/evaluator_registry.rs`

**Steps:**
1. Run `just milestone-p2`.
2. Run the existing focused tests:
   - `cargo nextest run -p leaven-evidence --test pairwise`
   - `cargo nextest run -p leaven-population --test tournament`
   - `cargo nextest run -p leaven-engine --test evaluator_registry`
3. Compare observed coverage against the P2 spec bullets.

**Expected Result:**
- Current example and ordinary focused tests pass.
- Missing property coverage is identified before edits.

## Task 2: Add Pairwise Cache-Key Property Coverage

**Files:**
- Modify: `crates/leaven-engine/tests/evaluator_registry.rs`

**Claim:** Reversing an ordered pairwise request changes cache identity, so `(A, B)` must not pool with `(B, A)` when `PairOrder::Ordered`.

**Test Shape:**
Add an integration test with a deterministic registered pairwise evaluator:

```rust
#[test]
fn ordered_pairwise_registry_cache_keeps_reversed_pairs_distinct() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let cases = CaseSet::new(vec!["case"]);
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .evaluator(OrderedPairwiseEvaluator)
            .build();
        let left = engine.insert_seed(TextArtifact("left".to_owned()), 0).unwrap();
        let right = engine.insert_seed(TextArtifact("right".to_owned()), 1).unwrap();
        let mut optimizer = ReversedOrderedPairCacheOptimizer { left, right };

        engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(engine.view().evaluation_request_count(), 2);
        assert_eq!(engine.view().assessment_count(), 2);
    });
}
```

The evaluator must use `CachePolicy::Deterministic`, and the optimizer must issue both ordered pairwise requests through `ctx.evaluate(EvaluatorId::PAIRWISE_JUDGE, ...)`.

**Verification:**
- Run `cargo nextest run -p leaven-engine --test evaluator_registry ordered_pairwise_registry_cache_keeps_reversed_pairs_distinct`.

## Task 3: Add Tournament Property Coverage

**Files:**
- Modify: `crates/leaven-population/tests/tournament.rs`

**Claims:**
- Generated finite learning rates and generated judgment sequences never produce non-finite abilities.
- A sequence where candidate X always beats Y never ranks Y above X.

**Test Shape:**
Use `proptest` at the population layer:

```rust
proptest! {
    #[test]
    fn generated_pairwise_updates_keep_abilities_finite(
        rate in 0.001_f64..1.0,
        judgments in proptest::collection::vec(any_pairwise_judgment(), 1..64),
    ) {
        let left = CandidateId::new();
        let right = CandidateId::new();
        let mut fit = BradleyTerryFit::new(FiniteF64::new(rate).unwrap());

        for judgment in judgments {
            fit.observe_pairwise(left, right, judgment);
            prop_assert!(fit.ability(left).as_f64().is_finite());
            prop_assert!(fit.ability(right).as_f64().is_finite());
        }
    }
}
```

Add a second property where every generated sequence length is one or more and every observation is `PairwiseJudgment::Left`; assert `fit.best() == Some(left)` and `fit.ability(left) >= fit.ability(right)` after each step.

**Verification:**
- Run `cargo nextest run -p leaven-population --test tournament`.

## Task 4: Add P2 End-To-End Integration Test

**Files:**
- Create: `crates/leaven/tests/pairwise_tournament.rs`

**Claim:** The umbrella public surface can run P2 end to end: create a second candidate, dispatch a registered pairwise judge, store pairwise evidence by reference, update `TournamentPopulation`, emit population events, and return the tournament winner as `RunResult.best`.

**Implementation Shape:**
- Mirror `examples/p2_pairwise_tournament/src/main.rs` but as a scenario test.
- Assert:
  - best artifact is `"aaa"`,
  - `engine.view().evaluation_request_count() == 1`,
  - `engine.view().assessment_count() == 1`,
  - graph assessment target is pairwise `(seed, contender)`,
  - `ctx.assessment_evidence(assessment_id)` returns `PairwiseJudgment::Right`,
  - callback or graph events include `EvaluationRequested`, `EvaluationCompleted`, `PopulationUpdated`, and `OptimizationEnded`.

**Verification:**
- Run `cargo nextest run -p leaven --test pairwise_tournament`.

## Task 5: Update Testing Documentation If The Suite List Changes

**Files:**
- Modify: `docs/testing/README.md`

**Claim:** The durable test contract should name the P2 test suites so future agents know the direct proof path.

**Implementation Shape:**
- Add `crates/leaven/tests/pairwise_tournament.rs` to the current suite list if Task 4 adds it.
- Keep the existing milestone command docs intact.

**Verification:**
- Check referenced paths exist.

## Task 6: Final Verification

**Commands:**
```bash
just milestone-p2
cargo nextest run -p leaven-evidence --test pairwise
cargo nextest run -p leaven-population --test tournament
cargo nextest run -p leaven-engine --test evaluator_registry
cargo nextest run -p leaven --test pairwise_tournament
just test
just check
```

**Completion Criteria:**
- All commands pass.
- P2 success criteria from the milestone spec are directly tested.
- No coverage floor is lowered.
- `jj st` shows only intentional P2 plan/test/code/doc changes.
