# Layer 3 Engine/Eval Surface Requirements

Status: canonical Layer 3 public/private contract requirements.

This file is the exact contract a future implementor should satisfy for
`RunContext`, stage contexts, eval/dataset/environment lowering, trust, cache,
budget, evidence, errors, and tests. It is not a compatibility plan.

## Contract Vocabulary

- Public means reachable by optimizer authors, engine users, examples, or
  downstream crates.
- Private means crate-internal implementation detail or explicit test support.
- Finalizer means an API that invokes or accepts stage output and performs the
  durable engine side effects: budget, graph, evidence store, cache, events,
  errors, and checkpointing.
- Raw context means `ProposalContext`, `EvaluationContext`, `RenderContext`, or
  `MaterializeContext` as the capability value passed into a stage call.

The root invariant is unchanged: `RunContext` is the public mutation path into
`RunGraph`: `AGENTS.md:38-40`.

## `RunContext` Public Contract

### Public Read APIs

Required:

- `graph(&self) -> RunGraphView<'_, P>` returns a read-scoped view.
- `iteration(&self) -> Option<IterationId>` reports current engine iteration.
- `budget(&self) -> BudgetSnapshot` reports current budget state.

Evidence:

- Current `RunContext` already exposes graph, iteration, and budget:
  `crates/leaven-engine/src/context/run_context.rs:104-123`.
- `RunGraphView` carries a `ReadScope`: `crates/leaven-engine/src/graph/view.rs:19-42`.

Rules:

- Read APIs must not mutate graph or budget.
- `graph()` must respect the actor read scope selected by engine/trust policy.
- Reports and downstream code should hold IDs and re-query graph views, not copy
  graph-owned truth. The report principle is specified at
  `docs/specs/initial_library.md:1984-1996`.

Proof/tests:

- Read-scope tests for candidate, assessment, evaluation-request, and event
  views.
- Snapshot/restore tests proving ID-backed reports still resolve after
  persistence.

### Public Proposal Finalizers

Required:

- `propose(&mut self, proposer, request)` is the normal finalizer for
  `Proposer<P>`.
- `record_proposal_batch(stage, batch, cost)` may remain public only as a
  finalizer for optimizer authors who intentionally construct `ProposalBatch`
  values themselves.
- `apply_batch(batch_id)` and `apply_proposal(proposal_id)` are finalizers for
  graph application.

Evidence:

- `RunContext::propose` dispatches proposer output then records it:
  `crates/leaven-engine/src/context/run_context.rs:191-208`.
- `record_proposal_batch` charges budget, records the batch, emits proposal
  events, and checkpoints: `crates/leaven-engine/src/context/run_context.rs:156-188`.
- Apply methods record success/failure events and outcomes:
  `crates/leaven-engine/src/context/run_context.rs:210-263`.
- Proposal effect/provenance constructors already preserve create/change and
  causal/informational lineage:
  `crates/leaven-core/src/proposal.rs:20-48`,
  `crates/leaven-core/src/proposal.rs:61-145`,
  `crates/leaven-core/src/proposal.rs:282-387`.

Rules:

- Public proposal finalizers must charge before graph mutation when the proposal
  stage reports cost.
- Proposal application failures produce `ApplyFailed` and error events, never
  silent skips.
- `ProposalEffect::Create` is for fresh authored artifacts; `ProposalEffect::Change`
  is for changes to existing candidates. The validation laws are specified at
  `docs/specs/initial_library.md:4554-4583`.
- `InfoRef::Assessment` records evidence/assessment references a proposer read;
  it is not causal lineage.

Proof/tests:

- Proposal finalization test with nonzero cost, emitted `BudgetCharged`,
  `ProposalBatchProduced`, and `ProposalRecorded` events.
- Invalid proposal effect/provenance laws tested at graph insertion/application.
- A custom optimizer can manually construct a batch and still get the same
  finalization side effects through `record_proposal_batch`.

### Public Evaluation Finalizers

Required:

- `evaluate(evaluator_id, request)` dispatches through the engine evaluator
  registry.
- `evaluate_with(evaluator, request)` dispatches a supplied static evaluator.
- Both finalizers enforce trust, resolve eval sets, build resolved request
  semantics, check cache, record evaluation request truth, dispatch the
  evaluator, charge budget, store evidence, record assessments, emit events, and
  checkpoint.

Evidence:

- Current static evaluation path performs these steps:
  `crates/leaven-engine/src/context/run_context.rs:378-440`.
- Dyn registry path mirrors it:
  `crates/leaven-engine/src/context/run_context.rs:442-501`.
- Evidence storage and assessment recording happen in
  `record_assessments`: `crates/leaven-engine/src/context/run_context.rs:609-637`.
- Current `RunContextError` includes trust, missing case set, missing evidence
  store, store, evaluation, and budget variants:
  `crates/leaven-engine/src/context/run_context.rs:695-737`.

Rules:

- Trust refusal must happen before request recording when the request is not
  authorized.
- Evaluation request recording must make both the original expression and
  resolved set visible in graph truth.
- Assessment shape must match request shape:
  `docs/specs/initial_library.md:4519-4527`.
- Evidence payloads are stored through `EvidenceStore`; graph assessments hold
  `EvidenceRef`, not inline payloads:
  `crates/leaven-store/src/evidence.rs:8-10`,
  `crates/leaven-engine/src/graph/storage.rs:95-130`.

Proof/tests:

- Unknown evaluator refusal without graph mutation.
- Trust refusal before request recording.
- Evaluator failure records request and stage error but no assessment.
- Evidence-store failure records request and store error but no assessment.
- Independent, pairwise, and listwise assessments record correct graph targets.

### Public Render/Materialize Finalizers

Required:

- Add `RunContext::render_with(stage, renderer, value, target)` or equivalent.
- Add `RunContext::materialize_into(stage, materializer, value, workspace)` or
  equivalent.
- The finalizers construct raw contexts, call the stage, charge returned
  `Metered` cost, emit success/failure events, preserve read scope, and
  checkpoint where state changed.

Evidence:

- The spec includes `RunContext::render`: `docs/specs/initial_library.md:1876-1883`.
- Rendering/materialization are costful and distinct:
  `docs/specs/initial_library.md:2257-2274`.
- Current traits return `Metered`: `crates/leaven-engine/src/stage/renderer.rs:8-26`.
- Current `RunContext` has raw constructors but no finalizers:
  `crates/leaven-engine/src/context/run_context.rs:312-325`.

Rules:

- Direct public raw-context construction must not be the supported way to run
  render/materialize work.
- A materializer must not write evidence from forbidden partitions:
  `docs/specs/initial_library.md:4585-4598`.
- Stage-owned sub-rendering/materialization must either use a stage-context
  finalizer that charges immediately or be included in the outer stage's returned
  `Metered` cost. The codebase must have one explicit law.

Proof/tests:

- Nonzero-cost render/materialize charges central budget.
- Hidden evidence is omitted from rendered/materialized outputs.
- Failure emits typed error events and does not claim success.

### Public Checkpoint/Private Optimizer State

Required:

- `checkpoint_with_optimizer_state` remains the public hook for optimizer-owned
  continuation state that cannot be derived from graph truth.
- Long-running/resumable optimizer implementations must implement
  `CheckpointableOptimizer` before product-ready resumability claims.

Evidence:

- `RunContext::checkpoint_with_optimizer_state` writes explicit optimizer state
  with graph/budget/cache state:
  `crates/leaven-engine/src/context/run_context.rs:125-141`.
- `CheckpointableOptimizer` and private state policy are defined at
  `crates/leaven-engine/src/stage/optimizer.rs:26-80`.
- GEPA optimizer surface requires private state such as RNG, sampler cursor,
  selector state, gate/admission state, merge scheduler state, and population
  state when not graph-derivable:
  `docs/specs/gepa_optimizer_surface.md:543-550`.

Rules:

- Graph truth is public run truth; optimizer private state is continuation data.
- Restore must fail before continuing if private state is unavailable or
  incompatible.

Proof/tests:

- Checkpoint includes graph, cache, budget, and optimizer private state.
- Restore validates that private-state candidates still exist in graph truth.

## Stage Context Contract

### Shared Rules

Required:

- Stage contexts are capability carriers passed to stage implementations by
  finalizers.
- They must not mutate graph.
- They may expose read-scoped graph views, budget snapshots or handles,
  render/materialize helpers, and scoped evidence readers according to actor
  policy.

Evidence:

- Capability separation is specified at
  `docs/specs/initial_library.md:1918-1937`.
- Current `ProposalContext` and `EvaluationContext` carry graph, budget handle,
  and read scope: `crates/leaven-engine/src/context/proposal_context.rs:8-62`,
  `crates/leaven-engine/src/context/evaluation_context.rs:8-63`.

Rules:

- Raw stage context constructors are private.
- Stage context APIs must be named by capability, not convenience.
- A stage context that has a `BudgetHandle` may charge explicit subwork; a
  context that has only a snapshot cannot charge and must rely on finalization of
  returned `Metered` cost.

Proof/tests:

- Visibility tests ensure downstream crates cannot construct raw contexts.
- Unit tests for stage context accessors live close to engine internals.

### `ProposalContext`

Required public capabilities when received by a proposer:

- scoped graph view;
- read scope;
- budget snapshot and charge-capable stage budget handle;
- render/materialize helpers that preserve proposer read scope;
- scoped evidence loading or rendering for visible assessments, if the chosen
  reflection contract permits proposer evidence access.

Evidence:

- Proposer trait request design expects rich views from `ctx.graph()`:
  `docs/specs/initial_library.md:2172-2198`.
- Current `ProposalContext` lacks evidence loading:
  `crates/leaven-engine/src/context/proposal_context.rs:8-62`.
- Typed evidence loading currently lives only on `RunContext`:
  `crates/leaven-engine/src/context/run_context.rs:640-652`.

Rules:

- Proposers cannot apply proposals or mutate graph.
- Proposers may record what they read only by returning proposals whose
  provenance includes `InfoRef` values.
- If proposer evidence access is enabled, it must check both assessment
  visibility and `EvidenceVisibility`.

Proof/tests:

- A proposer can read visible evidence and emit a proposal informed by it.
- The same proposer cannot read hidden assessment evidence.

### `EvaluationContext`

Required public capabilities when received by an evaluator:

- scoped graph view;
- budget snapshot and charge-capable stage budget handle;
- render/materialize helpers with evaluator read scope;
- no graph mutation.

Evidence:

- Current evaluator trait receives `ResolvedEvaluationRequest` and
  `EvaluationContext`: `crates/leaven-engine/src/stage/evaluator.rs:27-33`.
- Current `EvaluationContext` carries graph, budget handle, and read scope:
  `crates/leaven-engine/src/context/evaluation_context.rs:8-63`.

Rules:

- Evaluators receive resolved requests, never unresolved expressions.
- Evaluators return `Metered<Vec<Assessment<P>>>`.
- Unsupported request shape or granularity must be a typed evaluator refusal, not
  silent substitution.

Proof/tests:

- Evaluator cannot see hidden optimizer-only state unless trust allows it.
- Unsupported granularity produces typed error.

### `RenderContext` And `MaterializeContext`

Required public capabilities when received by a renderer/materializer:

- read-scoped graph view;
- read scope;
- budget snapshot;
- charge-capable handle only if the context is allowed to charge subwork.

Evidence:

- Current `RenderContext` has a budget handle internally but exposes only a
  snapshot: `crates/leaven-engine/src/context/render_context.rs:8-40`.
- Current `MaterializeContext` has only a snapshot:
  `crates/leaven-engine/src/context/materialize_context.rs:8-50`.

Rules:

- Rendering is a view, not truth transformation.
- Materialization is workspace side effect, not ordinary LM prompt rendering.
- Lossy rendering must be explicit.
- Materializers must be idempotent within a workspace when called with the same
  input.

Proof/tests:

- Materializer determinism/idempotence.
- Materializer cannot write forbidden evidence.
- Renderer/materializer cost is charged through finalizer or explicit subcharge.

## Eval/Dataset/Environment Lowering Contract

Required crate boundaries:

- `leaven-core` owns cold evaluation algebra:
  `EvaluationSet`, `EvaluationRequest`, `ResolvedEvaluationRequest`,
  `AssessmentGranularity`, `EvaluationPurpose`, and assessment shapes.
- `leaven-engine` owns execution:
  `Evaluator`, evaluator registry, `RunContext::evaluate`, cache, graph
  mutation, trust, budget, and evidence storage.
- `leaven-eval` owns lowered eval data:
  dataset, splits, split-use rules, request templates, suites, and report schemas.
- Environment/task/workspace semantics stay outside `leaven-eval`, in agentic or
  workspace/domain crates.

Evidence:

- Boundary table: `docs/specs/eval_lowering_detail.md:24-65`.
- `leaven-core` request/assessment algebra:
  `crates/leaven-core/src/evaluation.rs:29-224`,
  `crates/leaven-core/src/evaluation.rs:226-405`.
- Current `leaven-eval` module map is partial:
  `crates/leaven-eval/src/lib.rs:7-19`.

Rules:

- Product builders lower train/validation/test intent into `Dataset`,
  `DatasetSplits`, `SplitUsePolicy`, engine `CaseSet`, engine
  `EvaluationRequest`s, and engine `TrustPolicy`.
- `leaven-eval` must not depend on `leaven-engine`, `leaven-run`,
  `leaven-gepa`, workspace, agentic, LM, or provider crates.
- Single-task/no-dataset evaluation must be explicit, not faked as a hidden
  singleton dataset unless the suite chooses that law.
- Environments are evaluator/domain concerns, not dataset concerns.

Proof/tests:

- Topology contract for `leaven-eval` dependencies.
- Dataset/split/request-template/suite/report contract tests.
- Product lowering smoke that constructs engine requests and trust policy from
  eval suite data.

## Trust Contract

Required:

- Trust must enforce actual resolved membership and split use, not only
  syntactic partition references.
- `ReadScope` must hide forbidden assessment/evaluation-request records and
  evidence payloads consistently.
- Evaluators may see scorer-only/hidden targets when configured; proposers and
  optimizers may not consume hidden final-test data by default.

Evidence:

- Trust separation requirement:
  `docs/specs/guiding_principles.md:186-193`.
- Split-use lowering requirement:
  `docs/specs/eval_lowering_detail.md:344-397`,
  `docs/specs/eval_lowering_detail.md:744-757`.
- Current trust expression check:
  `crates/leaven-engine/src/trust.rs:119-182`.
- Current explicit case resolution:
  `crates/leaven-engine/src/case_set.rs:64-70`.

Rules:

- Hidden partition checks run after resolution.
- `EvaluationPurpose` and `SplitUsePolicy` participate in authorization.
- `EvidenceVisibility` must apply to evidence loading/rendering/materialization,
  not only graph record visibility.

Proof/tests:

- Hidden explicit case ID refusal.
- Dynamic set hidden-membership refusal.
- Final-test allowed only by final-report policy.
- Evidence visibility levels enforced by evidence reader/renderer/materializer.

## Cache Contract

Required:

- Default no-cache.
- Deterministic caching only with explicit evaluator fingerprint and cache
  policy.
- Cache keys include all semantics the evaluator may depend on.
- Cache hits preserve graph/request truth.

Evidence:

- Default no-cache:
  `crates/leaven-engine/src/cache.rs:9-21`.
- Current key omits request semantics:
  `crates/leaven-engine/src/cache.rs:46-59`.
- Current key builder:
  `crates/leaven-engine/src/context/run_context.rs:781-824`.
- Current cache-hit report:
  `crates/leaven-engine/src/context/run_context.rs:537-568`.

Rules:

- Key fields must include evaluator fingerprint, cache policy, resolved case-set
  version, resolved case IDs, request kind, candidate identities, granularity,
  purpose if evaluator-visible, pair/list order semantics, and assessment shape.
- Missing cache identity bypasses deterministic cache.
- Cache hit events/reports must show reuse lineage or create graph-visible
  request-local assessment aliases.

Proof/tests:

- Collision tests for shape, granularity, purpose, and order.
- Missing candidate cache identity bypass.
- Cache hit graph lineage test.

## Budget Contract

Required:

- Budget is infrastructure, not stopper policy.
- Every spending stage charges central budget.
- Budget exceeded errors identify stage, dimension, attempted charge, and current
  snapshot.

Evidence:

- Budget bookkeeping requirement:
  `docs/specs/guiding_principles.md:195-199`.
- Budget product contract:
  `docs/specs/gepa_public_private_surface.md:840-875`.
- Current ledger:
  `crates/leaven-engine/src/budget.rs:42-80`.
- Current budget handle:
  `crates/leaven-engine/src/budget.rs:90-115`.
- Current `RunContext::charge` events:
  `crates/leaven-engine/src/context/run_context.rs:265-284`.

Rules:

- Proposal/evaluation/render/materialize finalizers must charge returned
  `Metered` cost.
- Nested substage charges use explicit stage IDs.
- Negative costs or silent refunds are not allowed.
- Budget failure before graph mutation must leave no partial graph write except
  explicit error events.

Proof/tests:

- Budget exhausted before proposal recording.
- Budget exhausted during evaluation after request but before assessment.
- Materialization cost uses central budget.

## Evidence Contract

Required:

- Core keeps `P::Evidence` opaque.
- Stores persist evidence payloads and graph records hold refs.
- Standard evidence crates export only real production vocabulary.
- Reflection/proposal can consume scoped visible evidence by one explicit
  contract.

Evidence:

- Core assessment carries `P::Evidence`:
  `crates/leaven-core/src/evaluation.rs:354-405`.
- Store trait:
  `crates/leaven-store/src/evidence.rs:8-10`.
- Current graph records evidence refs:
  `crates/leaven-engine/src/graph/storage.rs:95-104`.
- `leaven-evidence` has both real starts and placeholder root exports:
  `crates/leaven-evidence/src/casewise.rs:36-78`,
  `crates/leaven-evidence/src/pairwise.rs:17-91`,
  `crates/leaven-evidence/src/lib.rs:1-77`.

Rules:

- Evidence is not preference.
- Evidence is not report truth until projected by report logic.
- Empty placeholder evidence types must not be root-exported as production
  vocabulary.
- `InfoRef::Assessment` records evidence read by a proposer; it does not make
  that assessment causal.

Proof/tests:

- Store round trip through `EvidenceStore` and graph `EvidenceRef`.
- Pairwise evidence and scalar/casewise evidence both drive separate preference
  or population tests.
- Placeholder export ledger removed or types implemented with laws/tests.

## Error Contract

Required:

- Known capability refusals have typed variants.
- Generic message variants remain only for unclassified edges.
- Failure policy decides continue/retry/abort; stage errors do not silently
  decide optimizer admission.

Evidence:

- Current `RunContextError` is already structured for many engine failures:
  `crates/leaven-engine/src/context/run_context.rs:695-737`.
- Current proposer/evaluator errors still rely on broad messages plus source:
  `crates/leaven-engine/src/stage/proposer.rs:92-125`,
  `crates/leaven-engine/src/stage/evaluator.rs:77-104`.
- Current run events encode error policy:
  `crates/leaven-engine/src/events.rs:25-30`,
  `crates/leaven-engine/src/events.rs:98-102`.

Rules:

- Add typed errors for unsupported request shape, unsupported granularity,
  invalid split use, hidden resolved membership, missing evidence visibility,
  materialization refusal, cache-key refusal, and checkpoint/private-state
  incompatibility.
- Preserve source chains for external failures.

Proof/tests:

- Exact error-variant tests for each refusal point.
- Event tests prove error policy and source chain are recorded.

## Test Contract

Required proof gates:

1. `RunContext` finalizer laws: proposal, apply, evaluation, render,
   materialize.
2. Trust laws: hidden partitions after resolution, split-use, evidence
   visibility.
3. Cache laws: semantic keys and graph-visible cache hits.
4. Eval lowering laws: dataset, splits, split-use, plan/request/suite/report
   without engine dependency.
5. Evidence/preference/population laws: scalar and pairwise paths through graph
   assessment IDs.
6. Optimizer expressibility: scalar keep-best, pairwise tournament, evidence-led
   reflective proposer, then GEPA.
7. Visibility tests: raw contexts are not public construction/run paths.

Evidence:

- Existing public finalizer tests are a good start:
  `crates/leaven-engine/tests/context_services.rs:28-49`,
  `crates/leaven-engine/tests/context_services.rs:124-208`.
- Existing raw-context tests show what must move inward:
  `crates/leaven-engine/tests/stage_trait_contracts.rs:17-98`.
- Existing split-use tests cover a lowered eval start:
  `crates/leaven-eval/tests/split_contract.rs:178-224`.

Rules:

- Tests should prove public/capability behavior unless a fact is genuinely
  private.
- No test should bless a proxy path as product proof.
- GEPA product proof is last, not first.

Completion gate:

`just check` remains the repo completion gate unless a future task explicitly
requests a narrower proof. The audit-specific readiness gate is that every proof
above has a focused test or compile-fail check before GEPA/AIME examples can be
called trustworthy.
