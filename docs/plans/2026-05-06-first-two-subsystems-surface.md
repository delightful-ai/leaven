# First Two Subsystems Surface Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the concrete proposal/candidate/run-graph and RunContext surface described by `docs/specs/first_two_subsystems.md`, reconciled against the newer `docs/specs/initial_library.md` v0.2.1a spec.

**Architecture:** Keep `leaven-core` as the cold core: typed artifacts, candidates, proposals, evaluation records, graph truth, context services, events, cost, errors, and stage traits. `RunGraph` remains append-only storage; `RunContext` becomes the only public mutation path. Where the two docs conflict, prefer the v0.2.1a spec in `initial_library.md`; this means no compatibility shim and no `ArtifactIdentity`/`ContentAddressed` split.

**Tech Stack:** Rust 2024, `thiserror`, `indexmap`, `uuid`, `chrono`, `serde`, focused unit/integration tests in `leaven-core`, and `cargo check --workspace`/`cargo test -p leaven-core` for verification.

---

## Grounding Already Done

- Read `docs/specs/initial_library.md` end to end.
- Read `docs/specs/first_two_subsystems.md` end to end.
- Inspected the current `leaven-core` scaffold.
- Verified current scaffold compiles with `cargo check --workspace`.

## Contract Decisions

- Keep `Artifact::content_id()` and `Artifact::apply(...)` as the canonical v0.2.1a trait shape. Do not introduce `ArtifactIdentity`, `ContentAddressed`, or `apply_change`.
- Do not add GEPA, workspace backends, real evaluator registries, or the runnable engine before the first two subsystems are stable.
- Treat `ProposalEffect::{Create, Change}` plus `ProposalProvenance { causal, informed_by }` as the hard graph contract.
- Validate proposal effect/provenance combinations before candidate insertion. Invalid combinations produce failed apply attempts and `ApplyFailed` events, not silent no-ops.
- Keep cost truth in `BudgetLedger`/`BudgetHandle`; proposal metadata is not cost truth.

## Todo

### 1. Add The First Graph Contract Tests

**Files:**
- Create: `crates/leaven-core/tests/graph_surface.rs`
- Touch only test support types inside the test file unless reuse clearly emerges.

**Tests to add first:**
- `create_proposal_creates_candidate_without_causal_parent`
- `change_proposal_requires_target_in_causal_inputs`
- `change_proposal_creates_causal_edge`
- `merge_proposal_records_pair_lineage_but_applies_to_one_target`
- `informed_by_does_not_affect_lineage`
- `same_content_can_have_multiple_candidates`
- `failed_apply_records_attempt`

**Verification:**
- Run `cargo test -p leaven-core graph_surface -- --nocapture`.
- Expected before implementation: compile failures or failing tests because graph mutators/views do not exist.

### 2. Land Report And Outcome Types

**Files:**
- Modify: `crates/leaven-core/src/context/run_context.rs`
- Modify: `crates/leaven-core/src/graph/storage.rs`
- Modify: `crates/leaven-core/src/error.rs`
- Re-export through `crates/leaven-core/src/prelude.rs` if public.

**Add:**
- `ProposalBatchReport`
- `ApplyReport`
- `ApplyOneReport`
- `ApplyOneOutcome`
- `EvaluationReport` skeleton if needed by stage signatures
- Context-level errors for unknown batch/proposal/evaluator and budget failures

**Verification:**
- Run `cargo check -p leaven-core`.

### 3. Implement RunGraph Proposal/Apply Mutators

**Files:**
- Modify: `crates/leaven-core/src/graph/storage.rs`
- Modify: `crates/leaven-core/src/graph/indices.rs` only if index shape needs adjustment.
- Modify: `crates/leaven-core/src/graph/events.rs` only if event payloads need exact alignment.

**Add `pub(crate)` methods:**
- `insert_seed`
- `record_proposal_batch`
- `apply_proposal_record`
- `record_population_events`
- `record_budget_event`
- `record_error`
- `record_event`

**Apply semantics:**
- Allocate `ApplyAttemptId` before applying so `CandidateOrigin::Proposal` has the real attempt id.
- `Create` inserts the artifact directly.
- `Change` clones the target artifact, calls `Artifact::apply`, then inserts the child candidate.
- Failed apply records exactly one failed attempt and no candidate.
- Applying a proposal twice is rejected.

**Verification:**
- Run `cargo test -p leaven-core graph_surface`.

### 4. Implement Proposal Validation Laws

**Files:**
- Modify: `crates/leaven-core/src/proposal.rs`
- Modify: `crates/leaven-core/src/graph/storage.rs`
- Modify: `crates/leaven-core/src/error.rs`

**Rules:**
- `Create + None` is valid.
- `Create + NAry` is valid.
- `Create + Single` is invalid.
- `Create + Pair` is invalid.
- `Change + Single(p)` is valid iff `target == p`.
- `Change + Pair(a, b)` is valid iff `target == a || target == b`.
- `Change + None` is invalid.
- `Change + NAry(xs)` is valid iff `xs.contains(target)`.

**Verification:**
- Add table-driven tests to `graph_surface.rs`.
- Run `cargo test -p leaven-core proposal_validation -- --nocapture`.

### 5. Implement RunGraphView Query Surface

**Files:**
- Modify: `crates/leaven-core/src/graph/view.rs`
- Add helper view structs in `crates/leaven-core/src/graph/view.rs` or `crates/leaven-core/src/graph/query.rs`.

**Add initial methods:**
- `candidate`
- `artifact`
- `content_id`
- `parents`
- `children`
- `siblings`
- `proposal_batch`
- `proposal_that_created`
- `informed_by`
- `informed`
- minimal assessment and pairwise query hooks if needed by later context tests

**Verification:**
- Existing graph contract tests must assert through `RunGraphView`, not private storage fields.

### 6. Replace Stage Marker Traits With Static Async Surfaces

**Files:**
- Modify: `crates/leaven-core/src/stage/proposer.rs`
- Modify: `crates/leaven-core/src/stage/evaluator.rs`
- Modify: `crates/leaven-core/src/context/proposal_context.rs`
- Modify: `crates/leaven-core/src/context/evaluation_context.rs`

**Add:**
- `Proposer<P>` with associated `Request` and `async fn propose(...) -> Result<Metered<ProposalBatch<P>>, ProposalError>`.
- `Evaluator<P>` with `async fn evaluate(ResolvedEvaluationRequest, EvaluationContext<'_, P>) -> Result<Metered<Vec<Assessment<P::Evidence>>>, EvaluationError>`.
- Minimal `ProposalContext` and `EvaluationContext` fields needed for graph view and budget handle.

**Verification:**
- Add compile-only dummy proposer/evaluator in tests.
- Run `cargo check -p leaven-core`.

### 7. Implement BudgetLedger And BudgetHandle

**Files:**
- Modify: `crates/leaven-core/src/cost.rs`
- Modify: `crates/leaven-core/src/context/run_context.rs`
- Modify: `crates/leaven-core/src/context/proposal_context.rs`
- Modify: `crates/leaven-core/src/context/evaluation_context.rs`

**Add:**
- `BudgetLedger`
- `BudgetExceeded`
- `BudgetHandle<'a>`
- `BudgetHandle::sub_stage`
- `RunContext::charge`

**Verification:**
- Unit test budget charge, remaining snapshot, and exceeded budget.
- Run `cargo test -p leaven-core budget`.

### 8. Implement RunContext Proposal/Apply Services

**Files:**
- Modify: `crates/leaven-core/src/context/run_context.rs`
- Modify: `crates/leaven-core/src/context/mod.rs`
- Modify: `crates/leaven-core/src/graph/events.rs` if event order needs adjustment.

**Add methods:**
- `graph`
- `iteration`
- `budget`
- `emit`
- `record_proposal_batch`
- `propose`
- `apply_batch`
- `apply_proposal`
- `record_population_events`

**Event rules:**
- `propose` emits `BudgetCharged`, `ProposalBatchProduced`, then one `ProposalRecorded` per proposal.
- `apply_proposal` emits exactly one of `ApplySucceeded` or `ApplyFailed`.
- `apply_batch` does not abort the batch after a single failed proposal.

**Verification:**
- Add `crates/leaven-core/tests/run_context_surface.rs`.
- Run `cargo test -p leaven-core run_context_surface -- --nocapture`.

### 9. Implement Minimal Evaluation Recording

**Files:**
- Modify: `crates/leaven-core/src/evaluation.rs`
- Modify: `crates/leaven-core/src/evidence.rs`
- Modify: `crates/leaven-core/src/graph/storage.rs`
- Modify: `crates/leaven-core/src/context/run_context.rs`

**Add:**
- Small `CaseSet`/resolution surface or a minimal resolver matching current `EvaluationSet`.
- `record_evaluation_request`
- `record_assessments`
- assessment indices for independent, pairwise, and listwise records
- in-memory `EvidenceStore` test implementation
- `RunContext::evaluate_with` without cache first

**Verification:**
- Tests for independent, pairwise, and listwise assessment indexing.
- Run `cargo test -p leaven-core evaluation`.

### 10. Add Cache And Trust Only After Evaluation Works

**Files:**
- Modify: `crates/leaven-core/src/context/trust.rs`
- Add or modify cache module depending on final placement.
- Modify: `crates/leaven-core/src/context/run_context.rs`
- Modify: `crates/leaven-core/src/graph/view.rs`

**Add:**
- Evaluation cache key over evaluator fingerprint, resolved request, candidate content ids, case-set version, and pair order.
- Default `CachePolicy::Never`.
- Trust checks for hidden partitions.
- Read-scope filtering in view/query methods where assessment evidence is exposed.

**Verification:**
- `Evaluate does not cache by default`
- `Deterministic evaluator cache hits`
- `Trust hides forbidden partition from proposer`

### 11. Add Golden Event Tests

**Files:**
- Modify: `crates/leaven-core/tests/run_context_surface.rs`
- Add expect-test snapshots if useful.

**Golden flows:**
- Proposal batch + successful apply.
- Proposal batch + failed apply.
- Evaluation requested + completed miss.
- Budget exceeded.

**Verification:**
- Run `cargo test -p leaven-core`.

### 12. Final Workspace Verification

**Commands:**
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test -p leaven-core`
- `cargo check --workspace`

**Completion condition:**
- The first two subsystem statements from `first_two_subsystems.md` are true for implemented code: proposals create or change one target, causal and informational provenance are distinct, candidates are graph-local occurrences, RunGraph is append-only truth, RunContext is the only mutation surface, costful context methods charge budget, major operations emit events, and evaluation evidence is stored by reference.

