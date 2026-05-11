# Layer 3 Optimizer-Author / Engine-User Audit

Date: 2026-05-11

Author: Codex audit pass over Layer 3 engine-author surfaces.

Scope:

- `crates/leaven-core`
- `crates/leaven-engine`
- `crates/leaven-store`
- `crates/leaven-evidence`
- `crates/leaven-eval`
- relevant specs under `docs/specs`

Question audited:

Can an optimizer author use `RunContext`, stage contexts,
graph/evidence/budget/trust/cache primitives directly and correctly, without
hidden missing accessors, sync-only seams, duplicated local shadows, or public
holes added just for tests?

Short answer: no, not yet.

The Layer 3 substrate is close enough to show the intended shape, but it still
has public seams that let engine users bypass the same invariants the specs say
`RunContext` must enforce. The highest-risk problems are not broad missing
modules. They are narrow public holes where a smart implementor can reasonably
choose an exported API and accidentally skip evidence persistence, cache
identity, trust enforcement, budget events, or graph recording.

## Findings

### L3-001: Raw Stage Context Factories Expose A Public Invariant Bypass

- `id`: `L3-001`
- `severity`: high
- `surface`: Layer 3 optimizer author / stage contexts
- `evidence`:
  - `docs/specs/first_two_subsystems.md:1503` says `RunContext` is the only
    public mutation path into `RunGraph`.
  - `docs/specs/first_two_subsystems.md:1504` says all context methods enforce
    budget, trust, events, cache, evidence-store, and error normalization.
  - `docs/specs/first_two_subsystems.md:1538` to
    `docs/specs/first_two_subsystems.md:1542` say `RunContext::propose` builds
    `ProposalContext`, calls the proposer, charges budget, records the batch,
    and emits events.
  - `docs/specs/first_two_subsystems.md:1580` to
    `docs/specs/first_two_subsystems.md:1587` say `RunContext::evaluate_with`
    checks trust, resolves the evaluation set, applies cache, calls the
    evaluator, charges budget, stores evidence, records assessments, and emits
    events.
  - `crates/leaven-engine/src/context/run_context.rs:290` publicly exposes
    `RunContext::proposal_context`.
  - `crates/leaven-engine/src/context/run_context.rs:303` publicly exposes
    `RunContext::evaluation_context`.
  - `crates/leaven-engine/tests/stage_trait_contracts.rs:24` creates a raw
    `ProposalContext`, and `crates/leaven-engine/tests/stage_trait_contracts.rs:27`
    to `crates/leaven-engine/tests/stage_trait_contracts.rs:29` call
    `DynProposer::propose_boxed` directly.
  - `crates/leaven-engine/tests/stage_trait_contracts.rs:74` creates a raw
    `EvaluationContext`, and `crates/leaven-engine/tests/stage_trait_contracts.rs:91`
    to `crates/leaven-engine/tests/stage_trait_contracts.rs:92` call
    `DynEvaluator::evaluate_boxed` directly.
- `promised behavior`: optimizer authors should drive proposal and evaluation
  work through `RunContext` so graph mutation, events, budget, trust, cache, and
  evidence storage are centralized.
- `actual behavior`: public raw context factories let external code construct
  stage contexts and call stage traits directly. Direct proposer calls return a
  `Metered<ProposalBatch<P>>` but do not record the batch, apply graph mutation,
  emit proposal events, checkpoint, or charge through the `RunContext::propose`
  finalizer. Direct evaluator calls return `Metered<Vec<Assessment<P>>>` but do
  not record an evaluation request, apply cache policy, store evidence by ref,
  insert assessment records, emit `EvaluationCompleted`, or charge through
  `RunContext::evaluate`.
- `why it matters`: this is a public path that looks legitimate to an engine
  user. It exists partly to test object-safe adapters, but the exported API does
  not communicate "test-only" or "non-finalizing." A future optimizer author can
  write a custom loop that appears to use Leaven engine primitives while
  silently bypassing the exact substrate Leaven is meant to guarantee.
- `correction direction`: hard cut over to finalizing public paths. Make raw
  context constructors crate-private, feature-gated test support, or clearly
  wrapped in an internal dispatch module. Public engine-author flow should be
  `RunContext::propose`, `RunContext::evaluate`, or a similarly named finalizer
  that consumes metered stage output and records all required graph, cache,
  evidence, budget, and event side effects.
- `target doc suggestion`: `internals/layer-3-engine-author/stage-contexts.md`

### L3-002: Proposer Context Can See Evidence Refs But Cannot Load Scoped Evidence Payloads

- `id`: `L3-002`
- `severity`: high
- `surface`: Layer 3 evidence / proposer context
- `evidence`:
  - `docs/specs/first_two_subsystems.md:1689` to
    `docs/specs/first_two_subsystems.md:1697` sketches `ProposalContext` as the
    proposer-facing surface and leaves room for renderers, evidence store, and
    related services.
  - `docs/specs/first_two_subsystems.md:2048` to
    `docs/specs/first_two_subsystems.md:2053` require graph views, renderers,
    evidence queries, and forbidden partition evidence to respect read scope.
  - `crates/leaven-engine/src/context/proposal_context.rs:8` to
    `crates/leaven-engine/src/context/proposal_context.rs:12` show
    `ProposalContext` only stores graph, budget, and read scope.
  - `crates/leaven-engine/src/context/proposal_context.rs:27` to
    `crates/leaven-engine/src/context/proposal_context.rs:62` expose graph,
    read scope, budget, render context, and materialize context, but no evidence
    reader.
  - `crates/leaven-engine/src/graph/view.rs:205` to
    `crates/leaven-engine/src/graph/view.rs:210` expose read-scoped assessment
    records.
  - `crates/leaven-engine/src/graph/view.rs:422` to
    `crates/leaven-engine/src/graph/view.rs:425` expose `EvidenceRef`.
  - `crates/leaven-engine/src/context/run_context.rs:640` to
    `crates/leaven-engine/src/context/run_context.rs:652` provide the current
    typed evidence payload accessor, but only on `RunContext`.
- `promised behavior`: proposer-stage code should have an honest, scoped way to
  use visible feedback, trace, and assessment evidence when building proposal
  requests.
- `actual behavior`: `ProposalContext` can reveal that evidence exists by
  exposing assessment views and evidence refs, but it cannot retrieve typed
  payloads. The only direct typed accessor is `RunContext::assessment_evidence`,
  which is not available inside `Proposer::propose`.
- `why it matters`: real reflective optimizers need scored feedback, traces,
  rationales, per-case failures, attribution, or agent transcripts. Without a
  scoped evidence reader in proposer context, an optimizer author must either
  avoid the engine proposer seam, preload evidence manually into proposer
  requests, or introduce another ad hoc escape hatch. This directly weakens the
  "engine user can build their own optimizer" surface.
- `correction direction`: make the evidence flow explicit. Either add a scoped
  evidence-reading/materialization capability to `ProposalContext`, respecting
  `ReadScope::visible_evidence` and hidden partitions, or document and enforce
  that optimizers must lower evidence into complete owned proposer requests
  before invoking a proposer. Do not leave both patterns implicit.
- `target doc suggestion`: `internals/layer-3-engine-author/evidence-trust-budget-cache.md`

### L3-003: Hidden Partition Trust Can Be Bypassed With Explicit Case IDs

- `id`: `L3-003`
- `severity`: high
- `surface`: Layer 3 trust / eval splits
- `evidence`:
  - `crates/leaven-core/src/evaluation.rs:50` to
    `crates/leaven-core/src/evaluation.rs:58` define `EvaluationSet::Partition`
    and `EvaluationSet::Cases`.
  - `crates/leaven-engine/src/trust.rs:154` to
    `crates/leaven-engine/src/trust.rs:182` collect hidden partitions from
    partition-shaped expressions but treat `Cases`, `Tagged`, `Recent`, and
    `Unscoped` as non-partition references.
  - `crates/leaven-engine/src/case_set.rs:64` to
    `crates/leaven-engine/src/case_set.rs:70` resolve explicit case IDs by
    checking only that each case exists.
  - `docs/specs/eval_lowering_detail.md:675` to
    `docs/specs/eval_lowering_detail.md:678` explicitly warn that
    train/validation/test requests must use `EvaluationSet::Partition`, not
    `EvaluationSet::Cases(test_ids)`, until engine trust can map explicit case
    IDs back to hidden partition membership.
  - `docs/specs/eval_lowering_detail.md:752` to
    `docs/specs/eval_lowering_detail.md:757` repeat that split-scoped product
    paths must use partition requests until this engine trust gap is closed.
- `promised behavior`: hidden validation/test partitions should be enforceable
  by engine trust policy so optimizers and proposers cannot accidentally use held
  out data.
- `actual behavior`: `TrustPolicy::check_evaluation_request` rejects requests
  that name hidden partitions, but does not map explicit case IDs back to hidden
  partition membership after resolution. A direct optimizer author can request
  hidden test cases as `EvaluationSet::Cases(vec![...])` and pass trust checks.
- `why it matters`: Layer 3 users are exactly the people likely to assemble
  evaluation requests manually. The code currently relies on higher-level
  product builders and human discipline to avoid a trust bypass. That is not an
  engine-level invariant.
- `correction direction`: move the hidden-membership check into engine
  resolution, or make `CaseSet` expose partition membership so trust can reject
  resolved sets containing hidden case IDs. Until then, any public engine-author
  docs should explicitly state that split-sensitive code must use partition
  expressions and that explicit case IDs are unsafe under hidden partitions.
- `target doc suggestion`: `internals/layer-3-engine-author/evidence-trust-budget-cache.md`

### L3-004: Evaluation Cache Keys Omit Request Semantics

- `id`: `L3-004`
- `severity`: high
- `surface`: Layer 3 evaluation cache
- `evidence`:
  - `docs/specs/gepa_optimizer_surface.md:535` to
    `docs/specs/gepa_optimizer_surface.md:540` require engine-owned evaluator
    cache keys to include evaluator fingerprint, resolved evaluation set,
    request shape, and artifact cache identities.
  - `docs/specs/milestone_examples_behavioral_contract.md:228` to
    `docs/specs/milestone_examples_behavioral_contract.md:238` require cache
    keys to include evaluator fingerprint, policy, resolved cases, and
    candidates in request order unless unordered symmetry is declared.
  - `crates/leaven-engine/src/cache.rs:47` to
    `crates/leaven-engine/src/cache.rs:59` define `EvaluationCacheKey` as
    evaluator, policy, case-set version, case IDs, and candidate cache
    identities.
  - `crates/leaven-engine/src/context/run_context.rs:781` to
    `crates/leaven-engine/src/context/run_context.rs:794` build cache keys from
    only those fields.
  - `crates/leaven-engine/src/context/run_context.rs:819` to
    `crates/leaven-engine/src/context/run_context.rs:824` reduce request
    candidates to IDs, including pairwise `(left, right)`, but do not include
    request kind, granularity, purpose, or pair order as separate key fields.
- `promised behavior`: deterministic evaluation cache should only reuse
  assessments for semantically identical evaluation requests.
- `actual behavior`: the key omits request shape, granularity, purpose, and
  explicit pair-order semantics. Independent, listwise, pairwise, aggregate,
  per-case, validation, search, and final-test requests can collide when they
  share evaluator fingerprint, policy, case-set version, case IDs, and candidate
  cache identities.
- `why it matters`: this can produce false cache hits where a later request
  receives assessment IDs from a different evaluation shape. For optimizer
  authors, that corrupts population updates, acceptance decisions, validation
  reports, and graph interpretation while still reporting `CacheStatus::Hit`.
- `correction direction`: include the full resolved request semantics in
  `EvaluationCacheKey`: request kind, granularity, purpose if evaluator-visible,
  and pair-order/symmetry semantics. If some fields are deliberately excluded,
  that exclusion needs to be a named evaluator cache law, not an accidental
  omission.
- `target doc suggestion`: `internals/layer-3-engine-author/evidence-trust-budget-cache.md`

### L3-005: Renderer And Materializer Work Has No Public Finalizing RunContext Path

- `id`: `L3-005`
- `severity`: medium
- `surface`: Layer 3 renderer/materializer budget path
- `evidence`:
  - `docs/specs/first_two_subsystems.md:1641` to
    `docs/specs/first_two_subsystems.md:1642` say `RunContext::propose`,
    `evaluate`, and render methods are where costful user code enters and should
    all route through `charge`.
  - `docs/specs/initial_library.md:1876` to
    `docs/specs/initial_library.md:1883` specify a public async
    `RunContext::render` method.
  - `docs/specs/initial_library.md:2257` to
    `docs/specs/initial_library.md:2274` explain that rendering and
    materialization are both async, costful, and intentionally different
    operations.
  - `crates/leaven-engine/src/stage/renderer.rs:8` to
    `crates/leaven-engine/src/stage/renderer.rs:25` define `Renderer` and
    `Materializer` methods returning `Metered`.
  - `crates/leaven-engine/src/context/run_context.rs:313` to
    `crates/leaven-engine/src/context/run_context.rs:325` expose raw
    `render_context` and `materialize_context` constructors, but no finalizing
    render/materialize method.
  - `crates/leaven-engine/tests/materializer_contract.rs:40` to
    `crates/leaven-engine/tests/materializer_contract.rs:45` call
    `materialize_into` directly with `ctx.materialize_context()`.
  - `crates/leaven-engine/tests/materializer_contract.rs:205` to
    `crates/leaven-engine/tests/materializer_contract.rs:213` return metered
    materialization output, but the caller does not charge it through
    `RunContext`.
- `promised behavior`: cost-bearing render and materialize operations should
  have the same budget/event discipline as proposal and evaluation work.
- `actual behavior`: renderer/materializer traits are cost-bearing, but the
  public engine surface only gives callers raw contexts. There is no public
  `RunContext` method that calls a renderer/materializer, charges returned cost,
  emits events, and checkpoints at the same invariant boundary.
- `why it matters`: agentic and workspace-heavy optimizers rely on
  materialization. If authors call materializers directly, they can do expensive
  filesystem or sandbox preparation while the budget ledger and event stream
  remain unaware.
- `correction direction`: add first-class finalizing methods such as
  `RunContext::render_with` and `RunContext::materialize_into` that invoke the
  trait, charge returned `Metered` cost, and emit any durable events needed for
  debugging. Keep raw context constructors internal or document them as
  non-finalizing implementation details.
- `target doc suggestion`: `internals/layer-3-engine-author/stage-contexts.md`

### X-001: Public Evidence Placeholder Types Masquerade As Standard Evidence

- `id`: `X-001`
- `severity`: medium
- `surface`: cross-cutting evidence vocabulary observed during Layer 3 audit
- `evidence`:
  - `crates/leaven-evidence/src/lib.rs:1` says the crate is a skeleton.
  - `crates/leaven-evidence/src/lib.rs:36` to
    `crates/leaven-evidence/src/lib.rs:65` define empty public structs for
    `DiffEvidence`, `RenderedDiff`, `JsonEvidence`, `ListwiseRankingEvidence`,
    `RankingItem`, `MixedEvidence`, `RawScoreValue`, `ScoreAxis`, `ScorePoint`,
    `ScoreVectorEvidence`, and `StringEvidence`.
  - `crates/leaven-evidence/src/lib.rs:66` to
    `crates/leaven-evidence/src/lib.rs:77` re-export those names from the crate
    root.
  - `docs/specs/philosophy_compliance_cleanup.md:28` to
    `docs/specs/philosophy_compliance_cleanup.md:31` say skeletons are findings
    when they expose behavior that can mislead a caller or rot a boundary.
  - `docs/specs/initial_library.md:2035` to
    `docs/specs/initial_library.md:2037` say not to expose empty marker traits
    as placeholders; the same concern applies to empty public evidence shapes
    presented as standard vocabulary.
- `promised behavior`: `leaven-evidence` should provide standard evidence
  shapes and optional capability traits that optimizer authors can safely bind
  to.
- `actual behavior`: several public, root-re-exported evidence names carry no
  fields, laws, constructors, or behavior. They look like usable standard
  vocabulary but cannot preserve domain truth.
- `why it matters`: optimizer authors may choose these names because they appear
  canonical, then discover they need local shadows or ad hoc replacement types.
  That creates duplicate evidence vocabulary and undermines the engine's
  standard extension story.
- `correction direction`: hard cut over to only exporting real evidence shapes
  from `leaven-evidence` root and prelude. Move placeholder names into an
  explicit non-public scaffolding module, or implement the data/laws/tests before
  re-exporting them.
- `target doc suggestion`: `cross-cutting/stub-placeholder-ledger.md`

## Non-Findings / Healthy Checks

- `RunGraph` mutation remains crate-private. The storage type exposes graph
  records through view structs, and mutation methods are still not ordinary
  public API. The problem is not `&mut RunGraph` leaking; it is the raw stage
  context path around `RunContext` finalizers.
- `leaven-core` keeps evidence opaque by design. `crates/leaven-core/src/evidence.rs:3`
  to `crates/leaven-core/src/evidence.rs:29` intentionally define `Evidence` as
  a marker trait, and `crates/leaven-core/src/problem.rs:17` to
  `crates/leaven-core/src/problem.rs:24` direct mixed evidence shapes toward a
  problem-specific enum. That is consistent with the cold-core boundary. The
  placeholder issue belongs in `leaven-evidence`, not `leaven-core`.
- Static evaluator/proposer traits are async-capable. The Layer 3 sync-only
  concern found in this pass is not in `leaven-engine` stage traits; it is in
  higher-level `leaven-run` runner/scorer findings already recorded elsewhere.
  Engine `Evaluator` and `Proposer` are async-capable, with object-safe dyn
  wrappers.

## Implementation Notes For The Fix Pass

Start with the invariant-bypass seams before expanding new functionality:

1. Close or clearly internalize public raw stage context factories.
2. Add finalizing `RunContext` paths for every cost-bearing stage operation.
3. Decide the proposer evidence contract and encode it in code and docs.
4. Fix trust enforcement for resolved explicit case IDs.
5. Expand `EvaluationCacheKey` to include request semantics.
6. Remove or implement root-re-exported placeholder evidence types.

The durable report split I would use:

- `internals/layer-3-engine-author/stage-contexts.md`: `L3-001` and `L3-005`.
- `internals/layer-3-engine-author/evidence-trust-budget-cache.md`: `L3-002`,
  `L3-003`, and `L3-004`.
- `cross-cutting/stub-placeholder-ledger.md`: `X-001`.

I did not run broad verification for this report. This was a documentation
write based on the source/spec audit and the review-tree convention files.
