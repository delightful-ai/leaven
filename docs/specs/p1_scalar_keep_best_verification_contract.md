# P1 Scalar Keep-Best Verification Contract

Status: execution contract for the next implementation milestone.
Date: 2026-05-06.

This spec turns the remaining first-two-subsystems work into a concrete,
testable contract. It is subordinate to:

- `docs/specs/initial_library.md`
- `docs/specs/first_two_subsystems.md`
- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`

When this document and those specs disagree, preserve the corrected v0.2.1b
crate topology and the first-two-subsystems invariants. Do not add compatibility
paths.

## Goal

Complete and verify the first-two-subsystems contract through Prototype 1:
scalar keep-best single-task.

The milestone is complete only when the library proves this end to end:

```text
seed artifact
-> optimizer step
-> proposer returns metered mutation alternatives
-> RunContext charges and records proposal batch
-> RunContext applies proposals through RunGraph
-> evaluator scores applied candidates
-> evidence and assessment records are graph-backed by reference
-> KeepBest observes assessments
-> Engine returns the best candidate
```

This is the next useful proof point because it exercises
`Optimizer + RunContext + RunGraph` without pulling in pairwise tournament,
GEPA, LLM SDKs, agents, or workspace backends.

## Non-Negotiables

- `RunContext` remains the only public graph mutation path.
- `RunGraph` mutators remain crate-private.
- Every costful context method charges budget or fails before mutation.
- Every major operation emits durable events in a documented order.
- Every fallible boundary produces typed error records or typed errors.
- `ProposalEffect::Create` and `ProposalEffect::Change` remain the only
  proposal effects.
- Causal lineage and informational provenance remain distinct typed facts.
- Assessment evidence is stored by reference once the evidence store path
  exists; the graph must not become a large-evidence blob store.
- Coverage floor is not lowered to land this milestone.

## Required Types and Surfaces

### Core and Engine

- `RunContext::propose`
  - takes a proposer plus request
  - builds a `ProposalContext`
  - charges returned metered cost
  - records the returned `ProposalBatch`
  - emits proposal events
- `RunContext::evaluate_with`
  - takes an evaluator and an unresolved `EvaluationRequest`
  - resolves `EvaluationSet` before evaluator execution
  - passes `ResolvedEvaluationRequest` into the evaluator
  - applies evaluator cache policy
  - stores assessment records and evidence refs
  - emits evaluation events
- `RunContext::evaluate`
  - registry-based convenience can remain deferred unless the P1 integration
    test needs it
- `EvaluationRequestRecord`
  - graph record of the original request and its resolved set
- `AssessmentRecord<P>`
  - graph record of assessment target, evaluator, score/evidence ref,
    granularity, and cost/event provenance
- `EvaluationReport`
  - includes `EvaluationRequestId`, `ResolvedEvaluationSetId`,
    `AssessmentId`s, cost, and cache status
- `EvaluationCache`
  - integrated into `RunContext::evaluate_with`, not just defined as vocabulary
- `RunEvent`
  - proposal, apply, evaluation, budget, error, iteration, and completion events
    needed by P1
- `Engine::run`
  - runs an `Optimizer`
  - creates `RunContext`
  - emits run and iteration events
  - returns `RunResult`
- `StopReason`
  - at least `OptimizerDone`, `BudgetReached`, `BudgetExceeded`, and `Error`

### Store

- `EvidenceStore<P>`
  - stores evidence values and returns `EvidenceRef`
  - should not know `RunGraph`
- `InlineEvidenceStore<P>`
  - deterministic test and default in-memory implementation
  - belongs in `leaven-store-inline`

### Prototype 1 Domain

Reusable standard pieces:

- `ScalarEvidence`
  - finite scalar score evidence for single-objective tests and examples
  - refuses `NaN`, positive infinity, and negative infinity at construction
  - belongs in `leaven-evidence`
- `HigherScoreIsBetter`
  - stateless preference over `ScalarEvidence`
  - belongs in `leaven-preference`
- `KeepBest`
  - population that tracks the best candidate seen so far
  - belongs in `leaven-population`

Test-support pieces:

- `TextArtifact`
  - small artifact with append/replace changes
- `SimpleMutationProposer`
  - returns deterministic mutation alternatives
- `ScalarEvaluator`
  - scores text artifacts using a deterministic function
- `ScalarKeepBestOptimizer`
  - one or more optimizer steps proving P1 without GEPA

The test-support pieces may start inside integration-test support modules. Move
them into standard crates only when they become reusable library surface.

## Functional Requirements and Tests

### RunGraph Requirements

1. Create proposal creates candidate without causal parent.
   - Already covered by `crates/leaven-engine/tests/graph_surface.rs`.

2. Change proposal requires the target to appear in causal inputs.
   - Test: invalid `Change { target } + CausalInputs::None` records failed
     apply, creates no candidate, and emits `ApplyFailed` plus `Error`.

3. Change proposal creates parent-child lineage.
   - Already covered for `Single(parent)`.

4. Merge-style proposal records pair lineage but applies to one target.
   - Test: seed `A` and `B`; proposal is `Change { target: A }` with
     `CausalInputs::Pair(A, B)`; assert apply uses `A`, child parents are
     `[A, B]`, and both parents list the child.

5. `informed_by` does not affect causal lineage.
   - Already covered for candidate info refs.

6. Same content can have multiple candidate IDs.
   - Test: two valid `Create` proposals with equal artifact identity produce
     distinct `CandidateId`s and separate origins.

7. Applying the same proposal twice is rejected idempotently.
   - Test: first apply succeeds or fails; second apply records a failed attempt
     and creates no second candidate.

8. Graph is append-only.
   - Property test: random valid seed/proposal/apply sequences never decrease
     record counts and do not mutate previously observed candidate/proposal
     facts.

9. Event order is stable for proposal/apply.
   - Golden tests:
     - success: `BudgetCharged`, `ProposalBatchProduced`,
       `ProposalRecorded`, `ApplySucceeded`
     - failure: `BudgetCharged`, `ProposalBatchProduced`,
       `ProposalRecorded`, `ApplyFailed`, `Error`

### RunContext Requirements

1. `RunContext::propose` records and charges.
   - Test: dummy proposer returns `Metered<ProposalBatch>` with nonzero cost.
     Assert budget decreases, proposal records exist, and events are emitted.

2. Proposer error records stage error.
   - Test: dummy proposer returns a typed proposal error. Assert no proposal
     batch is inserted and an `Error` event is emitted. If the error can carry
     spent cost, assert that cost is charged.

3. `apply_batch` creates candidates for every successful proposal and does not
   abort on partial failure.
   - Test: mixed valid and invalid batch returns both outcomes.

4. Evaluation resolves sets before evaluator execution.
   - Test evaluator records the request it received; assert it receives
     `ResolvedEvaluationRequest`, never the unresolved `EvaluationSet`.

5. Evaluation stores assessment and evidence by reference.
   - Test inline store receives evidence; graph stores `EvidenceRef` and
     `AssessmentId`; graph view can recover the assessment metadata.

6. Deterministic cache is used.
   - Test same deterministic request twice; first call is miss and invokes
     evaluator, second call is hit and does not invoke evaluator.

7. Default no-cache behavior remains default.
   - Test same request twice with `CachePolicy::Never`; evaluator is invoked
     twice and two request records are created.

8. Trust and read scope are enforced.
   - Test proposer or evaluator context cannot observe hidden assessment
     partitions. It is not enough to carry `ReadScope`; `RunGraphView` must
     filter or reject forbidden reads.

9. Budget exhaustion stops costful context method before mutation.
   - Test budget smaller than proposer/evaluator cost. Assert error event and
     no proposal/evaluation mutation after failed charge.

10. Callback ordering is monotonic.
    - Test recorder callback sees run, iteration, proposal, apply, evaluation,
      population, budget, and completion events in order.

### P1 Scalar Keep-Best Requirements

1. A scalar evaluator can score candidates.
   - Test independent evaluation over applied candidates produces
     `ScalarEvidence`.
   - Unit test: scalar evidence refuses non-finite scores before preference or
     population code can observe them.

2. `HigherScoreIsBetter` orders scalar evidence.
   - Unit tests: higher score wins, equal scores tie deterministically, and
     preference code only accepts finite scalar evidence.

3. `KeepBest` observes assessment records and tracks best candidate.
   - Unit tests: first observation wins, higher score replaces, lower score does
     not replace, tie policy is explicit.

4. Engine runs optimizer to completion.
   - Integration test: seed `TextArtifact`, run scalar keep-best optimizer,
     assert returned best candidate is the highest scoring mutation.

5. P1 event stream is coherent.
   - Integration test asserts a complete event subsequence:
     `OptimizationStarted`, `IterationStarted`, `BudgetCharged`,
     `ProposalBatchProduced`, `ProposalRecorded`, `ApplySucceeded`,
     `EvaluationRequested`, `EvaluationCompleted`, `PopulationUpdated`,
     `IterationEnded`, `OptimizationEnded`.

## Crate Scope

Expected crates touched:

- `crates/leaven-core`
  - only for tightening evaluation/assessment domain types
- `crates/leaven-engine`
  - main work: graph records, run context services, cache integration, trust
    enforcement, callbacks, engine loop, reports, tests
- `crates/leaven-store`
  - evidence-store trait shape
- `crates/leaven-store-inline`
  - inline evidence store implementation
- `crates/leaven-evidence`
  - `ScalarEvidence`
- `crates/leaven-preference`
  - `HigherScoreIsBetter`
- `crates/leaven-population`
  - `KeepBest`
- `crates/leaven-std`
  - curated re-exports of standard pieces
- `crates/leaven`
  - umbrella/prelude updates if public imports change
- `docs/testing/README.md`
  - update current suites and coverage scope
- `Justfile`
  - keep the one-command coverage gate aligned with the real executable surface

Crates that should not be touched for this milestone unless a compile boundary
forces it:

- `leaven-gepa`
- future MIPRO/TextGrad/trace optimizer crates
- `leaven-lm-*`
- `leaven-agent-*`
- `leaven-agentic`
- workspace backend crates
- artifact backend crates
- future CUDA/Python domain adapter crates
- `leaven-dsrs`

## Follow-Up TODOs

- `DynRenderer` remains a planned stage-registry surface, but P1 must not
  expose it as an empty marker trait. Define the erased value/target/view
  contract, add stage trait contract tests, then expose it.

## Coverage Policy

The current `coverage_line_floor` and `coverage_branch_floor` live in the root
`Justfile`. Do not lower either floor to land this milestone.

Coverage must not use source-path ignore regexes. The gate measures the real
workspace denominator plus each milestone binary that exercises example-only
runtime paths.

- Empty map crates, pure re-export facades, marker traits, type aliases, and
  pure data carriers naturally add no executable denominator until behavior
  lands.
- Constructors, state transitions, comparison logic, cache decisions, budget
  mutation, storage mutation, graph mutation, workspace side effects, and
  command execution are behavior and need tests.
- If a crate gains runtime behavior, add the narrow contract test or milestone
  binary execution that proves it in the same change.
- Coverage policy changes must make the measured surface more truthful, not
  hide code paths.

Completion requires:

```text
just check
```

The full gate must pass: formatting, line-count lint, clippy, nextest, doctests,
and coverage.

## Definition of Done

This milestone is done when all of the following are true:

- The P1 scalar keep-best integration test passes.
- All RunGraph requirements above are covered by tests.
- All RunContext requirements above are covered by tests or explicitly deferred
  in this file with a reason and a follow-up target.
- Every new behavior-bearing crate/file is in the coverage gate.
- `docs/testing/README.md` names the new suites.
- `just check` passes without reducing the coverage floor.
