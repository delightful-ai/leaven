# leaven

*Optimize anything in Rust.*

Project name: **leaven**. Crate: `leaven` (umbrella) + `leaven-core`, `leaven-engine`, `leaven-std`, `leaven-workspace`, `leaven-derive`. Metaphor: the small biological culture you mix into a substrate, walk away from, and come back to find transformed.

> Status: v0.2.1a, pre-implementation patch.  
> Date: 2026-05-06.  
> Supersedes the v0.2.1 spec. Folds in the implementation-readiness review patches: lifetime fixes on `Proposer::Request`, explicit report types, `EvaluationSet` resolution boundary, Bradley-Terry rename, workspace cleanup lifecycle, `BudgetHandle` ownership shape, proposal validation laws, and assorted residual wording cleanup. The architecture is unchanged; this is the last polish before P0/P1 prototypes.  
> This is still not an API lock — but it is now ready to be coded against.

---

## 0. What Changed in This Pass

This pass keeps the main architectural direction:

> **Engine runs an Optimizer. Optimizer owns algorithm rhythm. RunContext provides shared services. RunGraph records truth. Populations/frontiers maintain live strategy state. Preference relations interpret evidence.**

It tightens several places where the first v0.1 draft was still ambiguous or under-specified:

1. **Cost is now infrastructure, not proposal metadata.**  
   Every stage invocation is metered. Proposal batches, evaluations, renderers, agent runtimes, cache misses, and custom optimizer work can all charge the central `BudgetLedger`.

2. **`ProposalAnnotations` remains typed; `MetadataBag` remains operational.**  
   There is no generic “note” field. Semantic proposal payloads live in `ProposalAnnotations`; debug/operational extras live in `MetadataBag`.

3. **Evaluation distinguishes independent, pairwise, and listwise requests.**  
   Pairwise comparison is one assessment over two candidates. Independent scoring of two candidates is two assessments. The request shape says which one is intended.

4. **Assessment granularity is explicit.**  
   GEPA needs per-case scores to build instance-wise Pareto frontiers. Some evaluators return only aggregate assessments. `AssessmentGranularity` makes this explicit.

5. **`EvaluationSet` and `Niche` are the names.**  
   `Cohort` and `Cell` are removed from the user vocabulary.

6. **The engine has an explicit shape.**  
   The engine owns graph, budget, evaluator registry, renderer registry, cache, callbacks, stoppers, trust policy, and run store. The optimizer owns the algorithm rhythm.

7. **Evaluator registry replaces single evaluator.**  
   Simple users configure one evaluator. Advanced optimizers may call multiple evaluators by ID: task scorer, pairwise judge, human judge, verifier, etc.

8. **Callback event surface is real.**  
   The spec includes a concrete `RunEvent` shape.

9. **Caching policy is explicit.**  
   Engine-owned cache; evaluator-declared cache policy; default no-cache.

10. **Async/dyn policy is explicit.**  
    Optimizers are static-first. Stages intended for registries use object-safe `Dyn*` wrappers returning boxed futures.

11. **Evidence and annotations are run-wide types.**  
    If a run mixes evidence or annotation shapes, the user defines an enum. This is deliberate and Rust-native.

---

## 0.1 What Changed in v0.2

The v0.1 second pass survived the conceptual stress tests. The corrections in this pass are local refinements that emerged when implementations were walked through end-to-end.

12. **`parents` moves from `ProposalBatch` to `Proposal`.**  
    Sibling proposals in a single batch can have different causal parents (cross-branch synthesis case). The batch carries `semantics + metadata`; each proposal carries its own `parents`.

13. **`Parents::None` and `Arity::None` are first-class.**  
    Brand-new authored artifacts (Meta-Harness pattern: agent writes a fresh harness from scratch each iteration) have no causal predecessor. The lineage is bibliographic via `informed_by`, not causal.

14. **`Renderer<P, T, Target>` and `WorkspaceRenderer<P, T>` are split trait families.**  
    Value-returning rendering (prompt context, JSON blob, debug HTML) and side-effecting workspace population (write files into a sandbox) have different shapes. Conflating them was awkward. Resolves open question 27.1.

15. **Fitted preference relations live on `Population` impls.**  
    Stateless preferences (cardinal-pareto, scalar, lexicographic, copeland) implement `PreferenceRelation`. Stateful/fitted preferences (Bradley-Terry over accumulated pairwise judgments) are owned by `TournamentPopulation` which fits its model in `observe_assessment`. The `PreferenceRelation` trait stays simple. Resolves open question 27.6.

16. **`ParetoFrontier::partition_filter` is a builder method.**  
    Frontiers can declaratively ignore observations from specific case-set partitions (e.g. only update from `SEARCH`, never from `TEST`). Replaces ad-hoc skip logic in optimizer step bodies.

17. **`informed_by` is a typed graph relation.**  
    Promoted from string-keyed `MetadataBag` access to a first-class graph query. Avoids the python-gepa stringly-typed metadata-parsing failure mode. Stored as a structured `Vec<InfoRef>` in `ProposalProvenance`; exposed via `graph.informed_by(c)` and `graph.informed(c)`.

18. **Merge canonicalization is documented.**  
    `apply(&self, change) -> Self` only sees one artifact, so for `Parents::Pair(a, b)` the change must canonicalize to one parent and embed cross-parent content. Spelled out in §5.5 and §20.

19. **`ContentId` collision-resistance is a hard trait law.**  
    Strengthened from "observational identity" to "MUST be a cryptographic hash of all observationally-relevant state" with a derive macro for safe-by-default impls.

20. **Workspace lifecycle has its own section (§16.5).**  
    `WorkspaceFactory`, `WorkspaceBackend`, and `Workspace` are explicit. Standard backends (Local, E2B, Docker, K8s, Git-worktree) are sketched. Agent runtimes are kept separate from workspaces — they take a workspace and run commands in it.

21. **Implementation plan reorders prototypes 2 and 3.**  
    Pairwise tournament (formerly P3) runs before GEPA parity (formerly P2). Pairwise stresses what is *new* in this design (Pairwise eval requests, fitted preference relations, tournament populations) and is therefore the more informative early test.

22. **Two coding-agent worked examples.**  
    `gskill` and Meta-Harness are spelled out end-to-end in §22.4 and §22.5 to demonstrate the abstractions on real research workloads.

---

## 0.2 What Changed in v0.2.1

v0.2 retained shapes from v0.1 that became lies once the new capabilities (`Parents::None`, agentic proposers, typed provenance) were added. v0.2.1 fixes those without changing the architecture.

23. **`Proposal` carries `ProposalEffect`, not a bare `Change`.**  
    `effect: ProposalEffect::{ Create { artifact } | Change { target, change } }`. A brand-new authored artifact is honestly `Create`, not a `Change` with `Parents::None` whose `change` field is meaningless. Kills the v0.2 awkwardness around Meta-Harness-style fresh authoring.

24. **`ProposalProvenance { causal, informed_by }` is typed.**  
    `informed_by` is no longer "metadata under the hood" — it's a structured field of typed `InfoRef`s (candidates, assessments, proposals, external refs). Graph queries derive from this directly. Removes the python-gepa-stringly-typed failure mode v0.2 was sliding back toward.

25. **`Proposer::Request` is an associated type.**  
    GEPA reflective mutation, merge, Meta-Harness, ComBE, and MIPRO acquisition all need different request shapes. A single universal `ProposalRequest<P>` would collapse to an enum or a metadata bag. Associated type matches the static-first proposer story already chosen in v0.1.

26. **`RunContext::apply_batch` and `apply_proposal`** replace `apply(parents, batch)`.  
    Per-proposal effects subsume the parents argument. Context just routes the proposal through.

27. **`ProposalBatchSemantics::Ordered` is removed.**  
    Multi-batch optimizer rhythm covers ordered-dependency cases. Re-add if a real prototype forces it.

28. **`Materializable` moves out of cold core.**  
    Conflicts with the rendering-separation principle. Now a stdlib convenience trait used by default `WorkspaceRenderer` impls. Custom layouts always go through `WorkspaceRenderer`.

29. **`RendererRegistry` is demoted; stage-owned renderers are the default.**  
    Most stages should hold their renderers as fields. The registry exists for cross-stage shared rendering and debug, not as the primary path.

30. **`ContentId` law is softened (no split).**  
    `content_id` stays mandatory on `Artifact`. Trait law clarified to "deterministic hash with negligible collision probability at run scale; the cache trusts it; user contract is don't lie." No `ArtifactId / ContentAddressed` split — premature option-creation for use cases that haven't appeared. `#[derive(Optimize)]` plus dev-mode `verify_cache_consistency` cover the safety story.

31. **`Arity` is a request hint, not a law.**  
    Describes what the optimizer should provide as input when the optimizer drives parent selection. Proposers may emit fewer or more proposals than `Arity` suggests, and may set causal inputs differently per-proposal.

32. **Constructor sugar for `Proposal`.**  
    `Proposal::mutate(target, change)`, `Proposal::merge(a, b, change)`, `Proposal::create(artifact)` builders cover the common cases in one call. Users rarely construct the full struct directly.

---

## 0.3 What Changed in v0.2.1a

A pre-implementation review flagged real Rust-mechanics issues and residual wording inconsistencies in v0.2.1. Fixed before P0/P1 coding.

33. **`Proposer::Request` is no longer required to be `'static`.**  
    The v0.2.1 spec said `type Request: Send + Sync + 'static`, but the Meta-Harness example wanted `HistoryProposalRequest<'a>` borrowing from the run graph. Resolved: requests should be owned/lightweight (just identify what to do — a `Vec<CandidateId>` plus a `k`, etc.); proposers construct rich views (`HistorySnapshot`) internally from `ctx.graph()`. The bound on `Request` is relaxed, and the worked examples are updated to construct their snapshots inside `propose`.

34. **`<P::Artifact as Artifact>::Change` is the canonical change type.**  
    `P::Change` was used as shorthand in some signatures but `OptimizationProblem` doesn't define a `Change` associated type — the change lives on `Artifact`. Signatures fixed throughout. No new associated type added (would duplicate).

35. **Report types defined explicitly.**  
    `ProposalBatchReport`, `ApplyReport`, `ApplyOneReport`, `EvaluationReport` were referenced but undefined. Now spelled out in §8.3. They return IDs and graph-backed views, not graph-owned values — the graph is the durable truth.

36. **`EvaluationSet` resolution boundary explicit.**  
    `RunContext::evaluate` accepts an `EvaluationRequest` containing an unresolved `EvaluationSet`; the context resolves it and passes a `ResolvedEvaluationRequest` to the evaluator. Cache keys use the resolved set ID + case-set version. The graph records both the original expression and the resolution.

37. **`informed_by` wording cleanup (§10.2).**  
    Stale text saying graph queries are "backed by typed metadata recorded at proposal time" was replaced. They're derived from `ProposalProvenance::informed_by` directly, which is the v0.2.1 win.

38. **`BradleyTerryPreference` renamed to `BradleyTerryFit`.**  
    The stdlib list still listed Bradley-Terry under stateless `PreferenceRelation`s, contradicting §15.1 which placed fitted models on `Population` impls. Fixed: `BradleyTerryFit` is a model object owned by `TournamentPopulation<BradleyTerryFit>`. Stateless graph aggregators (`CopelandPreference`, `BordaPreference`) stay where they were.

39. **`Workspace::cleanup()` is explicit, not Drop-driven.**  
    Async cleanup cannot be reliably awaited in `Drop`. The trait now has an explicit `async fn cleanup(self)`. `Drop` does best-effort local cleanup or marks the workspace abandoned; remote cleanup (E2B sandbox destroy, K8s container delete, git worktree removal) goes through `cleanup()`. Factories may run janitors for abandoned workspaces.

40. **`BudgetHandle<'a>` is the single budget access type.**  
    Multiple `&mut BudgetLedger` references across `ProposalContext`, `EvalHandle`, etc. would be borrow-hostile. Stages now receive `BudgetHandle<'a> { ledger: &'a mut BudgetLedger, stage: StageId }` — one type, one mutable borrow path, stage tag baked in.

41. **Proposal validation laws (§24).**  
    Cheap correctness checks before graph insertion: `Create + None` ok; `Create + NAry` ok (aggregate); `Create + Single` invalid; `Change + Single` requires `target == single parent`; `Change + Pair` requires `target ∈ pair`; `Change + None` invalid. These prevent bad lineage data from entering the graph.

---

## 1. Executive Summary

We are building a Rust library for writing optimizers over arbitrary artifacts whose behavior can be assessed.

The library should support GEPA-style reflective prompt evolution, but must not be a GEPA-only engine. It should also support MIPRO-like surrogate optimizers, TextGrad/Trace-style feedback propagation, MAP-Elites, island evolutionary code search, pairwise-tournament preference optimization, skill-library evolution, agentic proposers, recursive meta-optimization, and future optimizers we have not read yet.

The library’s cold core should not assume:

```text
candidates are text dictionaries
evaluation returns scalar scores
selection is Pareto
proposals are one-shot LLM calls
evidence is an agent trajectory
"accept/reject" is a universal candidate lifecycle
a frontier is always maintained
train/validation exists
rendering is precomputed
every optimizer has GEPA's loop shape
```

The core should provide:

```text
typed artifacts and fallible typed changes
graph-local candidates
proposal batches
independent / pairwise / listwise evaluation requests
opaque evidence
preference relations over evidence
populations/frontiers as optimizer-owned live state
explicit rendering as the bridge from opaque values to consumers
budget/cost accounting across all stages
callbacks/events
caching hooks
trust/capability boundaries for agentic stages
a first-class Optimizer trait for algorithm authors
```

GEPA is one optimizer value, not the engine. It is composed from smaller GEPA-specific strategies: candidate selector, component selector, batch sampler, proposer, gate, validation policy, population/frontier, and optional merge proposer.

---

## 2. Design Philosophy

### 2.1 The consumer model

The library has three first-class consumer groups.

#### End users optimizing something

They want a short, obvious path:

```rust
let result = optimize(seed)
    .cases(train_cases)
    .holdout(validation_cases)
    .evaluate(my_evaluator)
    .using(Gepa::default().with_lm(reflection_lm))
    .budget(Budget::metric_calls(500))
    .run()
    .await?;
```

They should not have to understand every internal trait.

#### GEPA customizers

They want to replace one part of GEPA:

```rust
let gepa = Gepa::default()
    .candidate_selector(ParetoFrequencyWeighted)
    .component_selector(WorstEvidenceComponent)
    .proposal_count(3)
    .gate(StrictImprovement)
    .population(ParetoFrontier::by_case())
    .merge(SystemAwareMerge::adaptive());
```

They should not have to write a new optimizer.

#### Optimizer authors

They want to implement a new optimizer from a paper or idea:

```rust
impl Optimizer<MyProblem> for MyOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, MyProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        // own the algorithm rhythm
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, MyProblem>,
    ) -> Option<CandidateId> {
        // choose final answer
    }
}
```

They must be first-class. If implementing TextGrad, MIPRO, pairwise tournaments, or AlphaEvolve requires contorting the algorithm into GEPA’s sequence, the design failed.

### 2.2 Rust-library design standard

The library should feel like a serious Rust crate:

```text
precise names
honest types
explicit failure
typed capability boundaries
typed events
minimal magic
clear object-safety policy
async by default
ergonomic builders
well-documented trait laws
examples that show how to implement real optimizers
```

The core should be small, but not artificially tiny. A few sharp concepts are better than fewer overloaded ones.

### 2.3 Model-legibility

A competent model should be able to read a paper and map its concepts to library concepts:

```text
candidate selection -> CandidateSelector
Pareto frontier -> ParetoFrontier
niche -> NicheDescriptor / MapElites
pairwise judge -> EvaluationRequest::Pairwise
Bradley-Terry -> BradleyTerryFit (model object on TournamentPopulation)
train/val split -> CaseSet partitions + EvaluationSet
```

Naming is not polish; it is infrastructure.

---

## 3. Nomenclature

| Domain concept | Use this name | Avoid / demote | Reason |
|---|---|---|---|
| Thing being optimized | `Artifact` | `Candidate` | Candidate is a run-local artifact state. Artifact is the domain value. |
| Typed modification | `Change` | `Rewrite` | “Rewrite” is text-biased. `Change` works for files, code, configs, weights, prompts. |
| Artifact state inside a run | `Candidate` | `Snapshot`, `Node` | Optimizer literature says candidate. Graph storage may call it node internally. |
| Content identity | `ContentId` | `SnapshotId` | Same content can be reached by different causal paths. |
| Graph-local candidate identity | `CandidateId` | `SnapshotId` | A candidate is an occurrence in a run. |
| Attempted change | `Proposal` | `Edge` | Proposal exists before apply. Edge exists only after apply succeeds. |
| Multiple proposals from one call | `ProposalBatch` | `Parallel proposals` | Preserves sibling alternatives from one context. |
| Evaluation request | `EvaluationRequest` | `Metric call` | Evaluation may be scalar, pairwise, listwise, mixed. |
| Evaluation result | `Assessment` | `Score` | Assessment can contain any evidence shape. |
| Granularity of assessment | `AssessmentGranularity` | implicit per-case/per-set behavior | GEPA needs per-case; some optimizers need aggregate. |
| Opaque evaluation payload | `Evidence` | `Trace`, `SideInfo`, `Feedback` | Those are specific evidence shapes or renderings. |
| “Which is better?” logic | `PreferenceRelation` | `Comparator`, `Score` | Evidence is not preference. Preference consumes evidence. |
| Live optimizer state | `Population` | `Archive` | Population matches evolutionary/search literature and is intuitive. |
| Non-dominated live set | `Frontier`, `ParetoFrontier` | generic `ArchivePolicy` | If it is a Pareto frontier, say so. |
| Frontier partition | `Niche` | `Cell`, `Slice::Niche` | Niche is the MAP-Elites / quality-diversity term. |
| Where to evaluate | `EvaluationSet` | `Slice`, `Cohort` | EvaluationSet is direct. Cohort is removed. |
| Chooses candidates to evolve | `CandidateSelector` | `ParentSelector` | Literature and GEPA say candidate selection. Method may return parents. |
| Cheap pre-validation screen | `Gate` | core `Decision` | Gate is local to an optimizer, not global graph state. |
| Full algorithm value | `Optimizer` | `SearchStrategy` | Optimizer is the domain word. |
| Opaque-to-visible bridge | `Renderer` / `RenderedView` | `make_reflective_dataset` | Rendering is consumer-specific, not GEPA-specific. |
| Typed proposal payload | `ProposalAnnotations` | `Meta` / `Claims` split | One typed semantic payload. Claims are a capability on annotations. |
| Debug/operational extras | `MetadataBag` | `Note` | Metadata is non-semantic, extensible, and not read by algorithms by default. |

---

## 4. Architecture Overview

### 4.1 One-sentence architecture

> The engine runs an optimizer over typed artifacts; the optimizer uses a context to apply proposals, request evaluations, compare candidates, render views, update populations, and record events into an append-only run graph.

### 4.2 Ownership split

#### Engine owns infrastructure

```text
RunGraph
BudgetLedger
EvaluatorRegistry
RendererRegistry
EvaluationCache
Callback list
Stopper list
RunStore / checkpointing
TrustPolicy
iteration envelope
external stoppers
RNG seed / run identity
```

#### Optimizer owns algorithm rhythm

```text
which candidates to mutate
whether to evaluate before proposing
whether to propose one or many candidates
which evaluation requests to issue
when to update a population/frontier
when to call merge/crossover
whether to use a gate
when it considers itself done
which candidate is best
```

#### Stages own side-effectful work

```text
evaluator runs artifact(s) against the world
proposer produces changes
renderer creates views for consumers
agent runtime operates in a workspace/sandbox
preference relation interprets graph evidence
```

### 4.3 Engine policy: structured envelope, flexible step

The engine has a structured lifecycle:

```text
optimization started
optimizer initialized
while not stopped:
    check external stoppers
    iteration started
    optimizer.step(ctx)
    iteration ended
optimization ended
```

Inside `optimizer.step(ctx)`, the optimizer may call context methods in any order:

```text
ctx.propose(...)
ctx.apply(...)
ctx.evaluate(...)
ctx.render(...)
ctx.compare(...)
ctx.record_population_events(...)
ctx.emit(...)
ctx.charge(...)
```

The engine provides the envelope. The optimizer drives the algorithm. Context methods centralize graph, budget, cache, trust, and callback correctness.

---

## 5. Core Concepts

### 5.1 `Artifact`

An artifact is the domain value being optimized.

```rust
pub trait Artifact: Clone + Send + Sync + 'static {
    type Change: Clone + Send + Sync + 'static;
    type ApplyError: std::error::Error + Send + Sync + 'static;

    /// A deterministic hash that the evaluation cache trusts as identity.
    /// Same observable content => same id (with collision probability negligible
    /// at run scale). The cache uses content_id for dedup; lying about it
    /// produces silently incorrect cache results. See §24 for the contract
    /// and §17 for caching mechanics.
    ///
    /// Use #[derive(Optimize)] for safe-by-default field hashing. In dev mode,
    /// enable verify_cache_consistency to catch contract violations.
    ///
    /// Content-addressed external handles (git commit hashes, IPFS CIDs,
    /// docker image digests) trivially satisfy this — the handle IS the hash.
    fn content_id(&self) -> ContentId;

    /// Apply a typed change. Must be functional: same artifact + same change
    /// either fails the same way or produces the same content identity.
    fn apply(&self, change: &Self::Change) -> Result<Self, Self::ApplyError>;
}
```

`Artifact` does not know about scores, evidence, traces, rationale, claims, cases, or rendering.

Optional capabilities:

```rust
pub trait Decomposable: Artifact {
    type ComponentId: Eq + Hash + Clone + Send + Sync + 'static;

    fn components(&self) -> Vec<Component<Self::ComponentId>>;

    fn change_component(
        id: Self::ComponentId,
        edit: ComponentEdit,
    ) -> Self::Change;
}
```

`Decomposable` supports prompt modules, skill files, graph nodes, or any component-addressed artifact.

**`Materializable` is not part of cold core.** Workspace materialization (writing an artifact's content into a workspace directory) is rendering, not artifact identity. The cold-core `Artifact` trait stays free of workspace concerns. A standard library `Materializable` convenience trait exists for the common case where an artifact has an obvious canonical filesystem layout, and standard `WorkspaceRenderer` impls use it when present:

```rust
// stdlib convenience (not cold core)
pub trait Materializable: Artifact {
    async fn materialize(
        &self,
        workspace: &mut WorkspaceView<'_>,
    ) -> Result<RenderReport, MaterializeError>;
}
```

For artifacts without an obvious canonical layout — or where multiple consumers need different layouts — the user writes a `WorkspaceRenderer<P, ArtifactType>` directly. See §13.

### 5.2 `ContentId` and `CandidateId`

These are distinct.

```rust
pub struct ContentId([u8; 32]);
pub struct CandidateId(Uuid);
```

`ContentId` means artifact content identity.  
`CandidateId` means occurrence in this run graph.

The same content can appear multiple times in the graph via different proposals. That preserves causal history.

### 5.3 `Candidate`

A candidate is a graph-local artifact state.

```rust
pub struct Candidate<A: Artifact> {
    pub id: CandidateId,
    pub content_id: ContentId,
    pub artifact: A,
}
```

Candidates are created by successful proposal application or by seeding the run.

### 5.4 `OptimizationProblem`

Use one bundle for run-associated types.

```rust
pub trait OptimizationProblem {
    type Artifact: Artifact;
    type Case: Send + Sync + 'static;
    type Evidence: Evidence;
    type ProposalAnnotations: Clone + Send + Sync + 'static;
}
```

This keeps strategy signatures legible:

```rust
impl Optimizer<MyProblem> for MyOptimizer { ... }
```

Mixed evidence or annotations are represented with user-defined enums:

```rust
pub enum MyEvidence {
    Score(ScoreVectorEvidence),
    Pairwise(PairwiseJudgmentEvidence),
    AgentTrace(AgentTrajectory),
}

pub enum MyAnnotations {
    None,
    Reflection(ReflectionAnnotations),
    Edit(EditAnnotations),
    Merge(MergeAnnotations),
}
```

This is deliberate. The run-wide types tell the truth about all shapes that may occur in the run.

### 5.5 `Proposal`

A proposal is one record of "do this thing, with this lineage and this rationale." It separates *what to do* (effect) from *what informed it* (provenance) from *how to interpret it* (annotations) from *operational extras* (metadata).

```rust
pub struct Proposal<P: OptimizationProblem> {
    pub effect: ProposalEffect<P>,
    pub provenance: ProposalProvenance,
    pub annotations: P::ProposalAnnotations,
    pub metadata: MetadataBag,
}
```

#### Effect: what this proposal does

```rust
pub enum ProposalEffect<P: OptimizationProblem> {
    /// Brand-new authored artifact, no apply target.
    /// Used when the proposer constructs the artifact directly rather than
    /// transforming an existing candidate.
    /// Examples: Meta-Harness fresh harness each iteration; MIPRO initial
    /// surrogate sampling; ensemble aggregates that combine N → 1.
    Create {
        artifact: P::Artifact,
    },

    /// Mutation applied to an existing candidate.
    /// Examples: GEPA reflective mutation, TextGrad per-variable updates,
    /// AlphaEvolve code edits, MuF/Edit, merge (canonicalized — see below).
    Change {
        target: CandidateId,
        change: <P::Artifact as Artifact>::Change,
    },
}
```

`Change` covers the dominant case. `Create` exists because faking "no parent" as a `Change` requires inventing a null artifact value and pretending the apply step is `null.apply(replace_with: x) -> x`, which is a lie.

#### Provenance: causal lineage and bibliographic influence

```rust
pub struct ProposalProvenance {
    pub causal: CausalInputs,
    pub informed_by: Vec<InfoRef>,
}

pub enum CausalInputs {
    /// No causal predecessor. The proposal is `Create`.
    None,

    /// One causal parent. Standard mutation.
    Single(CandidateId),

    /// Two causal parents. Merge/crossover.
    /// One is the apply target (recorded in ProposalEffect::Change.target);
    /// the change embeds content sourced from the other.
    Pair(CandidateId, CandidateId),

    /// N-ary causal inputs. Uncommon; for ensemble aggregates and similar.
    NAry(Vec<CandidateId>),
}

pub enum InfoRef {
    Candidate(CandidateId),
    Assessment(AssessmentId),
    Proposal(ProposalId),
    External(ExternalRef),
}
```

`causal` records what the proposal was *derived from* — these contributed to the new candidate's content_id. `informed_by` records what the proposer *read while deciding* — these did not contribute to the content_id. The distinction matters for cache correctness (informed-by candidates can change without invalidating downstream cache entries) and for graph queries ("what learnings are incorporated into this candidate's lineage" vs "what was used to construct it").

`informed_by` is a typed structured field, not metadata. Graph queries `graph.informed_by(c)` and `graph.informed(c)` derive directly from this. Implementations are expected to populate it honestly — agentic proposers that read prior candidates must record those reads.

#### Constructor sugar

Users rarely construct the full `Proposal` struct. The common cases are covered by builders:

```rust
impl<P: OptimizationProblem> Proposal<P> {
    /// Standard mutation. Sets effect = Change, causal = Single(target).
    pub fn mutate(
        target: CandidateId,
        change: <P::Artifact as Artifact>::Change,
    ) -> ProposalBuilder<P>;

    /// Merge of two candidates. Sets effect = Change { target: a, change },
    /// causal = Pair(a, b). The change must already embed content from b.
    pub fn merge(
        a: CandidateId,
        b: CandidateId,
        change: <P::Artifact as Artifact>::Change,
    ) -> ProposalBuilder<P>;

    /// Brand-new authored artifact. Sets effect = Create, causal = None.
    pub fn create(artifact: P::Artifact) -> ProposalBuilder<P>;

    /// Aggregate of N candidates into a new artifact. Sets effect = Create,
    /// causal = NAry(parents).
    pub fn aggregate(parents: Vec<CandidateId>, artifact: P::Artifact) -> ProposalBuilder<P>;
}

pub struct ProposalBuilder<P: OptimizationProblem> { /* … */ }

impl<P: OptimizationProblem> ProposalBuilder<P> {
    pub fn informed_by<I: IntoIterator<Item = InfoRef>>(self, refs: I) -> Self;
    pub fn annotations(self, ann: P::ProposalAnnotations) -> Self;
    pub fn metadata(self, bag: MetadataBag) -> Self;
    pub fn build(self) -> Proposal<P>;
}
```

Typical usage:

```rust
// GEPA reflective mutation
Proposal::mutate(parent, change)
    .informed_by([InfoRef::Candidate(parent)])
    .annotations(reflection_notes)
    .build()

// GEPA merge
Proposal::merge(a, b, change_with_content_from_b)
    .informed_by([InfoRef::Candidate(a), InfoRef::Candidate(b)])
    .build()

// Meta-Harness fresh harness
Proposal::create(new_harness_artifact)
    .informed_by(referenced_candidates.iter().map(|&c| InfoRef::Candidate(c)))
    .annotations(proposer_notes)
    .build()
```

#### Cost is not on the proposal

No cost field is required here. Costs are recorded through stage invocations and `BudgetLedger`. A proposal may optionally include cost allocation metadata for analysis, but cost truth lives in the ledger.

#### Merge canonicalization

`Artifact::apply(&self, change) -> Self` only sees one artifact. So `Proposal::merge(a, b, change)` produces:

- `effect: ProposalEffect::Change { target: a, change }` — applied to `a` only
- `causal: CausalInputs::Pair(a, b)` — both contributed

The change must already embed any content the merge proposer wanted to import from `b`. The merge proposer reads `b` via the run graph during proposal generation, extracts the relevant components, and packages their content into the change. The framework records `Pair(a, b)` for lineage queries; the apply step ratifies what the merge proposer constructed.

#### Annotations are typed

The core does not distinguish `Meta` from `Claims`. If annotations have predictions or behavioral claims, they implement capability traits:

```rust
pub trait HasPredictions<P: OptimizationProblem> {
    fn predictions(&self) -> &[Prediction<P>];
}
```

```rust
pub trait HasBehavioralClaims {
    fn should_fix(&self) -> &str;
    fn should_not_break(&self) -> &str;
    fn confidence(&self) -> Confidence;
}
```

MuF/Edit-style annotations:

```rust
pub struct EditAnnotations {
    pub rationale: String,
    pub rhetorical_strategy: String,
    pub should_fix: String,
    pub should_not_break: String,
    pub rollback_note: String,
    pub confidence: Confidence,
}
```

### 5.6 `MetadataBag`

Operational metadata is separate from typed annotations.

```rust
pub struct MetadataBag {
    pub fields: BTreeMap<MetadataKey, MetadataValue>,
}
```

Use metadata for:

```text
raw response refs
worker IDs
stdout/stderr blob refs
rendered prompt blob refs
human comments
hostnames
trace file locations
diagnostic breadcrumbs
```

Metadata is for debugging and observability. Optimizer logic should depend on typed annotations, not ad hoc metadata, unless explicitly designed otherwise.

### 5.7 `ProposalBatch`

Proposal batches are first-class. A batch groups proposals that came from one reflection context — one `propose()` call.

```rust
pub struct ProposalBatch<P: OptimizationProblem> {
    pub proposals: Vec<Proposal<P>>,
    pub semantics: ProposalBatchSemantics,
    pub metadata: MetadataBag,
}
```

Each proposal carries its own `effect` and `provenance`. The batch does not carry causal inputs; sibling proposals from one reflection context may have entirely different causal lineages (or none at all). The batch only records *that they came from one context* and how they should be evaluated relative to each other.

The **cost of creating the batch** is recorded as a stage cost by `ctx.propose(...)` or `ctx.charge(...)`.

```rust
pub enum ProposalBatchSemantics {
    /// Sibling alternatives from one context.
    /// All alternatives are evaluated independently if applied successfully.
    /// Cost is N×eval, not amortized — the framework does not deduplicate.
    Alternatives,

    /// Candidate pool; optimizer/engine may evaluate only a subset by budget.
    CandidatePool,
}
```

`Ordered` (sibling proposals where later ones depend on earlier ones) was considered but removed in v0.2.1. Multi-batch optimizer rhythm covers ordered-dependency cases — the optimizer issues one batch, applies, then issues another batch using the new candidates as parents. Re-introducing `Ordered` would require the framework to interleave application with proposal generation, which is the optimizer's responsibility, not the engine's.

Important distinction:

```text
Alternatives = multiple independent proposals from one call
atomic multi-edit = one proposal whose Change contains multiple operations
```

Example user change type:

```rust
pub enum AgentChange {
    Single(AgentPatch),
    PatchSet {
        patches: Vec<AgentPatch>,
        atomic: bool,
    },
}
```

### 5.8 `CaseSet`

Generalization mode is represented by explicit partitions.

```rust
pub struct CaseSet<C> {
    pub cases: IndexMap<CaseId, C>,
    pub partitions: BTreeMap<PartitionId, Vec<CaseId>>,
    pub tags: BTreeMap<Tag, Vec<CaseId>>,
    pub version: CaseSetVersion,
}
```

Reserved partitions:

```rust
PartitionId::TRAIN
PartitionId::VALIDATION
PartitionId::TEST
```

Modes:

```text
single-task: no case set, singleton case set, or EvaluationSet::Unscoped
multi-task: TRAIN partition only
generalization: TRAIN and VALIDATION, with trust boundaries controlling proposer access
true test: TEST, usually evaluator-only and not visible to proposer
```

### 5.9 `EvaluationSet`

An evaluation set is where/what to evaluate.

```rust
pub enum EvaluationSet {
    /// No dataset scope. Useful for single-task or evaluator-internal tasks.
    Unscoped,

    All,

    Partition(PartitionId),

    Cases(Vec<CaseId>),

    Tagged(Tag),

    Recent {
        window: Window,
    },

    Sample {
        of: Box<Self>,
        n: usize,
        seed: u64,
    },

    Stratified {
        of: Box<Self>,
        by: Tag,
        k: usize,
        seed: u64,
    },

    Union(Vec<Self>),

    Intersect(Vec<Self>),

    Difference(Box<Self>, Box<Self>),
}
```

Evaluation sets are resolved before reaching the evaluator:

```rust
pub struct ResolvedEvaluationSet {
    pub id: ResolvedEvaluationSetId,
    pub expr: EvaluationSet,
    pub case_ids: Vec<CaseId>,
    pub resolved_at: DateTime<Utc>,
    pub case_set_version: CaseSetVersion,
}
```

#### Resolution boundary

The boundary between unresolved and resolved sets is sharp:

```text
Optimizer constructs EvaluationRequest with EvaluationSet (possibly dynamic).
RunContext::evaluate resolves the set:
  - Static variants (All, Partition, Cases, Tagged) resolve trivially.
  - Dynamic variants (Recent, Sample, Stratified) compute case_ids ONCE
    against the current case-set version, then freeze into ResolvedEvaluationSet.
  - Compositional variants (Union, Intersect, Difference) resolve recursively.
RunContext records the ResolvedEvaluationSet in the graph alongside the
  original EvaluationSet expression — both are queryable.
RunContext passes a ResolvedEvaluationRequest to the evaluator. Evaluators
  do not see EvaluationSet expressions; they see resolved case_ids.
Cache key uses (evaluator_fingerprint, ResolvedEvaluationSetId, case_set_version,
  candidate content_ids). Dynamic sets at different times resolve to different
  ResolvedEvaluationSetIds and therefore different cache entries.
```

```rust
pub struct ResolvedEvaluationRequest<'a> {
    pub kind: ResolvedRequestKind,
    pub resolved_set: &'a ResolvedEvaluationSet,
    pub granularity: AssessmentGranularity,
    pub purpose: EvaluationPurpose,
}

pub enum ResolvedRequestKind {
    Independent { candidates: Vec<CandidateId> },
    Pairwise { left: CandidateId, right: CandidateId, order: PairOrder },
    Listwise { candidates: Vec<CandidateId> },
}
```

This means the same dynamic `EvaluationSet::Recent { window: Duration::hours(1) }` issued at iteration 5 and iteration 6 produces two different `ResolvedEvaluationSet`s — they're snapshots, not lazy queries. The graph remembers both expressions and both resolutions. Cache hits across iterations are possible only if the resolution actually matches.

### 5.10 `AssessmentGranularity`

GEPA needs per-case assessments to build instance-wise Pareto frontiers. Some evaluators only produce aggregate assessments. The request must say what is wanted.

```rust
pub enum AssessmentGranularity {
    /// One assessment for the whole resolved evaluation set.
    Aggregate,

    /// One assessment per case in the resolved evaluation set.
    PerCase,

    /// Return both aggregate and per-case assessments when possible.
    Both,
}
```

If an evaluator cannot provide the requested granularity, it returns an explicit `EvaluationError::UnsupportedGranularity`.

### 5.11 `EvaluationRequest`

Evaluation can be independent, pairwise, or listwise.

```rust
pub enum EvaluationRequest {
    Independent {
        candidates: Vec<CandidateId>,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
    },

    Pairwise {
        left: CandidateId,
        right: CandidateId,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
        order: PairOrder,
    },

    Listwise {
        candidates: Vec<CandidateId>,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
    },
}
```

This avoids ambiguity:

```text
Independent over [A, B] returns independent assessments for A and B.
Pairwise over (A, B) returns comparison assessment(s).
Listwise over [A, B, C] returns ranking/listwise assessment(s).
```

### 5.12 `AssessmentTarget`

An assessment has a target: unscoped, aggregate set, or case-specific.

```rust
pub enum AssessmentTarget {
    Unscoped,
    EvaluationSet(EvaluationSetId),
    Case {
        set: EvaluationSetId,
        case: CaseId,
    },
}
```

### 5.13 `Assessment`

An assessment is evaluation output.

```rust
pub enum Assessment<P: OptimizationProblem> {
    Independent {
        candidate: CandidateId,
        target: AssessmentTarget,
        evidence: P::Evidence,
        cost: Cost,
        metadata: MetadataBag,
    },

    Pairwise {
        left: CandidateId,
        right: CandidateId,
        target: AssessmentTarget,
        evidence: P::Evidence,
        cost: Cost,
        metadata: MetadataBag,
    },

    Listwise {
        candidates: Vec<CandidateId>,
        target: AssessmentTarget,
        evidence: P::Evidence,
        cost: Cost,
        metadata: MetadataBag,
    },
}
```

This model supports:

```text
aggregate scalar evaluation
per-case scalar evaluation
pairwise comparison per case
pairwise comparison across a set
listwise ranking
mixed evidence
human judge rationales
agent traces
compiler logs
```

### 5.14 `Evidence`

Evidence is opaque to core.

```rust
pub trait Evidence: Send + Sync + 'static {}
```

Optional capabilities:

```rust
pub trait RenderEvidence: Evidence {
    fn render(&self, ctx: RenderContext<'_>) -> RenderedView;
}
```

```rust
pub trait AttributedEvidence<C>: Evidence {
    fn invocations(&self) -> Vec<Invocation<C>>;
    fn evidence_for(&self, component: &C) -> Option<ComponentEvidence<'_>>;
}
```

```rust
pub trait CommandEvidence: Evidence {
    fn commands(&self) -> &[CommandRecord];
}
```

```rust
pub trait DiffEvidence: Evidence {
    fn diff_summary(&self) -> Option<RenderedDiff>;
}
```

The core does not require these. Strategies bind what they need.

### 5.15 `PreferenceRelation`

Evidence is not preference. A relation interprets evidence.

```rust
pub trait PreferenceRelation<P: OptimizationProblem>: Send + Sync {
    fn compare(
        &self,
        left: CandidateId,
        right: CandidateId,
        scope: PreferenceScope,
        graph: RunGraphView<'_, P>,
    ) -> Preference;
}
```

```rust
pub enum Preference {
    LeftBetter,
    RightBetter,
    Equivalent,
    Incomparable,
}
```

Standard relations:

```text
HigherScoreIsBetter
LowerScoreIsBetter
ParetoPreference
LexicographicPreference
CopelandPreference
BordaPreference
CondorcetPreference
UserDefinedPreference
```

(Fitted preference *models* like `BradleyTerryFit` are not in this list — they live on `Population` impls. See §15.1.)

`Score` is not a cold primitive. Scores are one evidence shape plus one preference relation.

### 5.16 `Population`

A population is live optimizer state.

```rust
pub trait Population<P: OptimizationProblem>: Send {
    fn insert_seed(
        &mut self,
        candidate: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent>;

    fn select_candidates(
        &mut self,
        arity: Arity,
        graph: RunGraphView<'_, P>,
    ) -> Vec<CandidateId>;

    fn observe_candidate(
        &mut self,
        candidate: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        Vec::new()
    }

    fn observe_assessment(
        &mut self,
        assessment: AssessmentId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        Vec::new()
    }

    fn best(
        &self,
        graph: RunGraphView<'_, P>,
    ) -> Option<CandidateId>;

    fn view(&self) -> PopulationView<'_>;
}
```

This supports:

```text
candidate-scored optimizers, where a population observes a candidate after validation
tournament optimizers, where a population observes pairwise/listwise assessments
streaming optimizers, where population changes as fresh assessments arrive
```

Standard populations:

```text
KeepBest
ParetoFrontier
MapElites
BeamPopulation
IslandsPopulation
TournamentPopulation
LenientParetoFrontier
NoveltyPopulation
NoPopulation
```

### 5.17 `Niche`

A niche is a frontier/population partition.

```rust
pub trait NicheDescriptor<P: OptimizationProblem>: Send + Sync {
    type Niche: Eq + Hash + Clone + Send + Sync + 'static;

    fn niche(
        &self,
        candidate: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Self::Niche;
}
```

MAP-Elites and related methods use niches. GEPA’s instance Pareto can be represented as a frontier keyed by case ID; that is a frontier partition, not an evaluation set.

### 5.18 Rendering

Rendering converts opaque core types into consumer-specific views — strings for prompts, JSON for typed signatures, HTML for human inspection, structured directories for agents to grep. Rendering may be async and costful.

The library splits rendering into two trait families:

- **`Renderer<P, T, Target>`** — value-returning. Used for prompts, summaries, JSON blobs, debug HTML.
- **`WorkspaceRenderer<P, T>`** — side-effecting. Populates a workspace by writing files. Used for materializing artifacts, lineage history, traces, and any structured filesystem layout an agentic stage will read.

Full trait definitions, examples, and trait laws live in §13. The framework does not pre-render anything; consuming stages choose their renderings.

---

## 6. Cost and Budget

### 6.1 Cost is infrastructure

Cost is not proposal metadata. Cost is tracked across all side-effectful stages.

```rust
pub struct Cost {
    pub axes: BTreeMap<CostUnit, Amount>,
}
```

Standard units:

```text
usd
wall_time_ms
cpu_time_ms
input_tokens
output_tokens
cached_input_tokens
llm_calls
tool_calls
metric_calls
subprocesses
```

User-defined units:

```rust
pub struct CostUnit(SmolStr);
```

### 6.2 Budget ledger

```rust
pub struct BudgetLedger {
    // internal
}

impl BudgetLedger {
    pub fn remaining(&self, unit: CostUnit) -> Option<Amount>;

    pub fn charge(
        &mut self,
        stage: StageId,
        cost: Cost,
    ) -> Result<(), BudgetExceeded>;

    pub fn snapshot(&self) -> BudgetSnapshot;
}
```

Every context operation that can spend cost records it.

### 6.3 Metered values

```rust
pub struct Metered<T> {
    pub value: T,
    pub cost: Cost,
}
```

Examples:

```text
Proposer produces Metered<ProposalBatch>
Evaluator produces Metered<Vec<Assessment>>
Renderer produces Metered<RenderedView>
AgentRuntime produces Metered<AgentTranscript>
```

Even if the public type stores cost on `Assessment`, the graph also records a `BudgetCharged` event for the stage invocation.

### 6.4 Cost truth

There are three levels:

1. **Stage invocation cost** — authoritative cost charged to the ledger.
2. **Assessment cost** — cost attributable to a returned assessment.
3. **Optional cost allocation** — approximate per-proposal or per-candidate attribution for analysis.

Only the ledger is authoritative.

---

## 7. Engine

### 7.1 Engine shape

```rust
pub struct Engine<P, O>
where
    P: OptimizationProblem,
    O: Optimizer<P>,
{
    problem: P,
    optimizer: O,

    evaluators: EvaluatorRegistry<P>,
    renderers: RendererRegistry<P>,

    graph: RunGraph<P>,
    budget: BudgetLedger,
    cache: EvaluationCache<P>,

    stoppers: Vec<Box<dyn DynStopper<P>>>,
    callbacks: Vec<Box<dyn DynCallback<P>>>,

    rng: StdRng,
    trust: TrustPolicy<P>,
    store: RunStore<P>,
}
```

Simple builders install one primary evaluator:

```rust
optimize(seed)
    .evaluate(my_evaluator)
```

Advanced users install multiple evaluators:

```rust
optimize(seed)
    .evaluator(EvaluatorId::PRIMARY, task_evaluator)
    .evaluator(EvaluatorId::PAIRWISE_JUDGE, pairwise_judge)
    .evaluator(EvaluatorId::HUMAN_REVIEW, human_review)
```

### 7.2 Engine run loop

```rust
impl<P, O> Engine<P, O>
where
    P: OptimizationProblem,
    O: Optimizer<P>,
{
    pub async fn run(mut self) -> Result<RunResult<P>, EngineError> {
        self.emit(RunEvent::OptimizationStarted { /* ... */ });

        {
            let mut ctx = self.context();
            self.optimizer.initialize(&mut ctx).await?;
        }

        loop {
            if let Some(reason) = self.check_stoppers() {
                self.emit(RunEvent::OptimizationStopping { reason });
                break;
            }

            let iteration = self.graph.next_iteration();
            self.emit(RunEvent::IterationStarted { iteration });

            let step_status = {
                let mut ctx = self.context_for_iteration(iteration);
                self.optimizer.step(&mut ctx).await?
            };

            self.emit(RunEvent::IterationEnded {
                iteration,
                status: step_status.clone(),
            });

            match step_status {
                StepStatus::Continue => {}
                StepStatus::Stop { reason } => {
                    self.emit(RunEvent::OptimizationStopping { reason });
                    break;
                }
            }
        }

        let best = self.optimizer.best_candidate(self.graph.view());
        self.emit(RunEvent::OptimizationEnded { best });

        Ok(RunResult {
            graph: self.graph,
            best,
            budget: self.budget.snapshot(),
        })
    }
}
```

### 7.3 Stop policy

External stoppers are checked before each optimizer step. The optimizer may also stop itself. Budget exhaustion may stop during any context operation.

```rust
pub enum StopReason {
    Stopper {
        id: StopperId,
        message: String,
    },

    Optimizer {
        message: String,
    },

    BudgetExceeded {
        unit: CostUnit,
        requested: Amount,
        remaining: Amount,
    },

    ErrorPolicy {
        message: String,
    },

    UserRequested,

    Composite(Vec<StopReason>),
}
```

The graph records all stop reasons.

---

## 8. Optimizer Author Surface

### 8.1 `Optimizer`

```rust
pub trait Optimizer<P: OptimizationProblem>: Send {
    async fn initialize(
        &mut self,
        ctx: &mut RunContext<'_, P>,
    ) -> Result<(), OptimizerError> {
        Ok(())
    }

    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, P>,
    ) -> Result<StepStatus, OptimizerError>;

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, P>,
    ) -> Option<CandidateId>;
}
```

```rust
pub enum StepStatus {
    Continue,
    Stop { reason: StopReason },
}
```

The optimizer owns algorithm rhythm.

### 8.2 `RunContext`

```rust
impl<'a, P: OptimizationProblem> RunContext<'a, P> {
    pub fn graph(&self) -> RunGraphView<'_, P>;

    pub fn budget(&self) -> BudgetSnapshot;

    pub async fn propose<Pr>(
        &mut self,
        proposer: &Pr,
        request: Pr::Request,
    ) -> Result<ProposalBatchReport<P>, ProposalError>
    where
        Pr: Proposer<P>;

    /// Apply every proposal in a batch. Returns per-proposal apply outcomes.
    /// Per-proposal effects (Create vs Change) and provenance are read from
    /// each Proposal directly; the batch is not parameterized by parents.
    pub async fn apply_batch(
        &mut self,
        batch: ProposalBatch<P>,
    ) -> Result<ApplyReport<P>, ApplyError>;

    /// Apply a single proposal. Convenience for optimizers that don't batch.
    pub async fn apply_proposal(
        &mut self,
        proposal: Proposal<P>,
    ) -> Result<ApplyOneReport<P>, ApplyError>;

    pub async fn evaluate(
        &mut self,
        request: EvaluationRequest,
    ) -> Result<EvaluationReport<P>, EvaluationError>;

    pub async fn evaluate_with(
        &mut self,
        evaluator: EvaluatorId,
        request: EvaluationRequest,
    ) -> Result<EvaluationReport<P>, EvaluationError>;

    pub fn compare(
        &self,
        left: CandidateId,
        right: CandidateId,
        scope: PreferenceScope,
        relation: &dyn DynPreferenceRelation<P>,
    ) -> Preference;

    pub async fn render<T, Target>(
        &mut self,
        value: &T,
        target: Target,
    ) -> Result<RenderedView, RenderError>;

    pub fn record_population_events(
        &mut self,
        population: PopulationId,
        events: Vec<PopulationEvent>,
    );

    pub fn emit(&mut self, event: RunEvent<P>);

    pub fn charge(
        &mut self,
        stage: StageId,
        cost: Cost,
    ) -> Result<(), BudgetExceeded>;
}
```

Context methods handle:

```text
graph writes
budget charges
cache lookup
callback emission
trust policy enforcement
error normalization
event metadata
persistence hooks
```

This is what makes `Optimizer` first-class without making optimizer authors reimplement the engine.

### 8.3 Report types

Context methods return small report structs. The reports carry IDs and graph-backed views — never graph-owned values — because the run graph is the durable truth and reports are read-only summaries of what was just recorded.

```rust
/// Returned by RunContext::propose. The batch was already recorded in the graph;
/// this report exposes IDs and the freshly-built batch for the caller's loop.
pub struct ProposalBatchReport<P: OptimizationProblem> {
    pub batch_id: ProposalBatchId,
    pub batch: ProposalBatch<P>,
    pub cost: Cost,
}

/// Returned by RunContext::apply_batch. Per-proposal apply outcomes.
/// Successful candidates are queryable via successful_candidates();
/// failed proposals' errors are in failed.
pub struct ApplyReport<P: OptimizationProblem> {
    pub batch_id: ProposalBatchId,
    pub outcomes: Vec<ApplyOneReport<P>>,
}

impl<P: OptimizationProblem> ApplyReport<P> {
    pub fn successful_candidates(&self) -> impl Iterator<Item = CandidateId> + '_;
    pub fn failed(&self) -> impl Iterator<Item = (ProposalId, &ErrorRecord)> + '_;
}

pub struct ApplyOneReport<P: OptimizationProblem> {
    pub proposal_id: ProposalId,
    pub outcome: ApplyOutcome<P>,
}

pub enum ApplyOutcome<P: OptimizationProblem> {
    Success {
        candidate: CandidateId,
        content_id: ContentId,
    },
    Failure {
        error: ErrorRecord,
    },
}

/// Returned by RunContext::evaluate. Assessment IDs (graph-owned) plus a
/// borrowed view for the caller's immediate use.
pub struct EvaluationReport<'a, P: OptimizationProblem> {
    pub request_id: EvaluationRequestId,
    pub resolved_set: ResolvedEvaluationSetId,
    pub assessment_ids: Vec<AssessmentId>,
    pub assessments: Vec<AssessmentView<'a, P>>,
    pub cost: Cost,
    pub cache: CacheStatus,
}
```

The principle: **reports point at the graph, they do not duplicate it.** If a caller wants persistent access to an assessment, they hold the `AssessmentId` and re-query via `ctx.graph().assessment(id)`. The borrowed views in reports are convenience for the immediate loop body.

---

## 9. Async and Dynamic Dispatch Policy

### 9.1 Static-first optimizer

`Optimizer` is static by default:

```rust
Engine<P, O: Optimizer<P>>
```

Optimizers are usually configured values, not registry items.

### 9.2 Dyn-friendly stages

Stages likely to live in registries get object-safe erased traits:

```rust
pub trait DynEvaluator<P: OptimizationProblem>: Send + Sync {
    fn evaluate_boxed<'a>(
        &'a self,
        request: EvaluationRequest,
        ctx: EvaluationContext<'a, P>,
    ) -> BoxFuture<'a, Result<Vec<Assessment<P>>, EvaluationError>>;
}
```

Equivalent wrappers:

```text
DynProposer
DynRenderer
DynPreferenceRelation
DynCallback
DynStopper
```

Adapters exist from static traits to dyn traits.

Core should not require `async_trait`, but an ergonomic adapter crate may use it.

### 9.3 Static traits may use async fn

Static traits may use `async fn` where they are not intended for dyn dispatch. Dyn wrappers use boxed futures.

---

## 10. Run Graph

### 10.1 Graph role

The graph is durable truth. It records what happened. It does not decide what is good.

It records:

```text
candidates
proposal batches
apply attempts
assessments
population events
budget charges
stage errors
cache hits/misses
callbacks/checkpoints
stop events
```

### 10.2 Required graph queries

```rust
impl<P: OptimizationProblem> RunGraphView<'_, P> {
    fn candidate(&self, id: CandidateId) -> Option<CandidateView<'_, P>>;

    fn artifact(&self, id: CandidateId) -> Option<&P::Artifact>;

    fn parents(&self, id: CandidateId) -> Vec<CandidateId>;

    fn children(&self, id: CandidateId) -> Vec<CandidateId>;

    fn lineage(&self, id: CandidateId) -> Lineage<'_, P>;

    fn siblings(&self, id: CandidateId) -> Vec<CandidateId>;

    /// Candidates this proposal read from during reflection.
    /// Distinct from causal parents: these candidates contributed to the proposer's
    /// decision but did not contribute to the new candidate's content_id.
    /// Derived from ProposalProvenance::informed_by recorded at proposal time;
    /// not from MetadataBag.
    fn informed_by(&self, id: CandidateId) -> Vec<CandidateId>;

    /// Inverse of informed_by: candidates whose proposers read from `id`.
    fn informed(&self, id: CandidateId) -> Vec<CandidateId>;

    fn proposal_batch(&self, id: ProposalBatchId) -> Option<ProposalBatchView<'_, P>>;

    fn proposal_that_created(&self, id: CandidateId) -> Option<ProposalView<'_, P>>;

    fn assessments(&self, id: CandidateId) -> AssessmentQuery<'_, P>;

    fn assessments_for_target(
        &self,
        id: CandidateId,
        target: AssessmentTarget,
    ) -> AssessmentQuery<'_, P>;

    fn pairwise_assessments(
        &self,
        left: CandidateId,
        right: CandidateId,
    ) -> AssessmentQuery<'_, P>;

    fn population_events(&self, population: PopulationId) -> Vec<PopulationEvent>;

    fn recent_failures(&self, window: Window) -> Vec<FailureRef>;

    fn costs(&self) -> CostSummary;

    fn candidate_tree(&self) -> CandidateTree<'_, P>;
}
```

Strategy authors should navigate by optimizer concepts, not raw storage maps.

---

## 11. Evaluator

```rust
pub trait Evaluator<P: OptimizationProblem>: Send + Sync {
    fn id(&self) -> EvaluatorId;

    fn fingerprint(&self) -> Fingerprint;

    fn cache_policy(
        &self,
        request: &EvaluationRequest,
    ) -> CachePolicy {
        CachePolicy::Never
    }

    fn pair_order_policy(&self) -> PairOrderPolicy {
        PairOrderPolicy::Ordered
    }

    async fn evaluate(
        &self,
        request: EvaluationRequest,
        ctx: EvaluationContext<'_, P>,
    ) -> Result<Metered<Vec<Assessment<P>>>, EvaluationError>;
}
```

Evaluators can be:

```text
deterministic functions
LLM judges
human judges
subprocess runners
agentic sandboxes
compiler/profiler harnesses
pairwise tournament judges
listwise rankers
```

A closure adapter should exist for simple cardinal evaluation.

---

## 12. Proposer

A proposer emits proposal batches. It is a stage used by optimizers such as GEPA, but not required by the engine.

```rust
pub trait Proposer<P: OptimizationProblem>: Send + Sync {
    /// The shape of input this proposer expects.
    /// GEPA reflective mutation, merge, Meta-Harness, ComBE, MIPRO acquisition,
    /// and human editors all need different request shapes — they are not the
    /// same data and should not be smashed into a universal enum.
    ///
    /// Convention: requests are owned and lightweight. They identify *what to do*
    /// (a candidate id, a list of case ids, a `k`), not *what data to use*.
    /// Rich views (HistorySnapshot, lineage walks, evidence aggregations) are
    /// constructed inside `propose` from `ctx.graph()`. This avoids lifetime
    /// gymnastics on the trait while keeping requests type-safe at the call site.
    type Request: Send + Sync;

    fn id(&self) -> ProposerId;
    fn arity(&self) -> Arity;

    async fn propose(
        &self,
        request: Self::Request,
        ctx: ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError>;
}

/// A hint for what causal-input shape this proposer expects when the optimizer
/// is responsible for parent selection. NOT a hard law — proposers may emit
/// proposals with different causal shapes than their declared arity (e.g.
/// fail and emit zero proposals; emit alternatives with different parents).
pub enum Arity {
    /// No causal parents required. Proposer authors brand-new artifacts.
    /// Examples: Meta-Harness, MIPRO initial sampling.
    None,

    /// One causal parent. Standard mutation case.
    Single,

    /// Two causal parents. Merge/crossover.
    Pair,

    /// Variable count; proposer decides at request time.
    Variadic,
}
```

Proposers can be:

```text
one-shot LLM calls
multi-stage typed pipelines
agentic workspace processes
merge/crossover algorithms
surrogate acquisition samplers
human editors
ensemble reducers
parallel scan aggregators
```

Use `ctx.propose(&proposer, request)` when possible so stage events and costs are recorded uniformly. The associated `Request` type means the call is fully type-checked: passing a `MergeRequest` to a `ReflectiveMutation` proposer is a compile error.

#### Dyn dispatch for proposers in registries

When proposers must live in a registry (rare; usually proposers are stage fields), the type-erased wrapper hides the request type:

```rust
pub trait DynProposer<P: OptimizationProblem>: Send + Sync {
    fn id(&self) -> ProposerId;
    fn arity(&self) -> Arity;

    /// The erased proposer accepts a type-erased request, downcasting internally.
    fn propose_boxed<'a>(
        &'a self,
        request: Box<dyn Any + Send>,
        ctx: ProposalContext<'a, P>,
    ) -> BoxFuture<'a, Result<Metered<ProposalBatch<P>>, ProposalError>>;
}
```

Static proposers are the default; the dyn wrapper is for runtime-loaded plugins.

---

## 13. Renderers

Rendering converts opaque values into consumer-specific views. Rendering may be async and costful.

The library splits rendering into two trait families because the side effects differ:

- **`Renderer<P, T, Target>`** returns a value. Used for prompt assembly, JSON blobs, debug HTML, summary strings.
- **`WorkspaceRenderer<P, T>`** populates a workspace by side effect. Used for materializing artifacts, lineage history, traces, and any large structured filesystem layout that an agentic stage will read.

Conflating the two was awkward (an `()` view type plus reliance on a `&mut Workspace` smuggled through the context). The split makes both shapes honest.

### 13.1 `Renderer<P, T, Target>` — value-returning

```rust
pub trait Renderer<P: OptimizationProblem, T, Target>: Send + Sync {
    type View;

    async fn render(
        &self,
        value: &T,
        target: Target,
        ctx: RenderContext<'_, P>,
    ) -> Result<Metered<Self::View>, RenderError>;
}
```

Examples:

```text
Artifact     -> ReflectionPrompt        (View = String)
Evidence     -> ReflectionSummary       (View = String)
Lineage      -> PromptContext           (View = StructuredPrompt)
RunGraph     -> HumanDebugHtml          (View = String)
CandidatePair -> PairwiseJudgePrompt    (View = JudgePromptDoc)
```

### 13.2 `WorkspaceRenderer<P, T>` — side-effecting

```rust
pub trait WorkspaceRenderer<P: OptimizationProblem, T>: Send + Sync {
    async fn render_into(
        &self,
        value: &T,
        workspace: &mut WorkspaceView<'_>,
        ctx: RenderContext<'_, P>,
    ) -> Result<Metered<RenderReport>, RenderError>;
}

pub struct RenderReport {
    pub files_written: usize,
    pub bytes_written: u64,
    pub truncations: Vec<TruncationNote>,
}
```

Examples:

```text
HarnessArtifact          -> writes harness.py into the workspace
ExecutionTrace           -> writes per-case trace files into traces/
LineageDirectorySnapshot -> writes a candidate-per-subdirectory tree
HistorySnapshot          -> the orchestrator that calls the above three
GitWorktreeRendering     -> ensures the worktree is at the parent commit
```

`WorkspaceView<'_>` is a borrowed handle into a workspace subtree, with `subdir`, `write_file`, `read_file`, and `run_command`. It respects the workspace's underlying backend (local fs, e2b sandbox, k8s container, git worktree). See §16.5 for workspace lifecycle.

### 13.3 Choosing between the two

If the consumer wants a value back (string for an LLM prompt, JSON for a typed signature, HTML for a viewer), use `Renderer`. If the consumer needs a directory tree it can `grep` and `cat` (agentic proposer, sandboxed evaluator, debugger reproducing a run), use `WorkspaceRenderer`. The same artifact can have both kinds of renderers attached for it.

### 13.4 Stage-owned renderers are the default

Most stages should hold their renderers as direct fields, not look them up through a registry:

```rust
pub struct ReflectiveMutation<R, L> {
    renderer: R,         // Renderer<P, ParentLineage, ReflectionPrompt>
    lm: L,
}

pub struct AgenticHarnessProposer<HR, AR> {
    history_renderer: HR,           // WorkspaceRenderer<P, HistorySnapshot>
    agent_runtime: AR,
}
```

Stage-owned composition keeps understanding local — the rendering used by a particular stage is visible at the type level — and avoids action-at-a-distance through a global table.

A `RendererRegistry` exists for cross-stage shared rendering (e.g. a debug viewer that wants to render arbitrary artifact types) and for plugin scenarios where rendering choices are made at runtime. It is not the central rendering mechanism. Most users will never touch it.

---

## 14. Preference Relations

```rust
pub trait PreferenceRelation<P: OptimizationProblem>: Send + Sync {
    fn compare(
        &self,
        left: CandidateId,
        right: CandidateId,
        scope: PreferenceScope,
        graph: RunGraphView<'_, P>,
    ) -> Preference;
}
```

Preference relations consume graph evidence and may be:

```text
pure functions over scalar evidence
Pareto relations over score-vector evidence
lexicographic relations
Copeland tournament aggregators (stateless aggregation over recorded judgments)
custom domain relations
```

Preference may be partial. `Incomparable` is a valid result.

**`PreferenceRelation` is stateless.** Stateful/fitted preference models (Bradley-Terry, Plackett-Luce, fitted human-preference aggregators) are owned by `Population` impls instead — typically `TournamentPopulation` — because their state needs to update as new pairwise/listwise observations arrive. See §15 for the population side of this.

---

## 15. Population and Frontier

A population is live optimizer state. A frontier is a kind of population.

```rust
pub trait Population<P: OptimizationProblem>: Send {
    fn id(&self) -> PopulationId;

    fn insert_seed(
        &mut self,
        candidate: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent>;

    fn select_candidates(
        &mut self,
        arity: Arity,
        graph: RunGraphView<'_, P>,
    ) -> Vec<CandidateId>;

    fn observe_candidate(
        &mut self,
        candidate: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        Vec::new()
    }

    fn observe_assessment(
        &mut self,
        assessment: AssessmentId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        Vec::new()
    }

    fn best(
        &self,
        graph: RunGraphView<'_, P>,
    ) -> Option<CandidateId>;

    fn view(&self) -> PopulationView<'_>;
}
```

Population events:

```rust
pub enum PopulationEvent {
    Inserted {
        population: PopulationId,
        candidate: CandidateId,
        reason: String,
    },

    Replaced {
        population: PopulationId,
        old: CandidateId,
        new: CandidateId,
        reason: String,
    },

    Removed {
        population: PopulationId,
        candidate: CandidateId,
        reason: String,
    },

    Ignored {
        population: PopulationId,
        candidate: CandidateId,
        reason: String,
    },

    Reweighted {
        population: PopulationId,
        candidate: CandidateId,
        weight: f64,
        reason: String,
    },

    Migrated {
        from: PopulationId,
        to: PopulationId,
        candidate: CandidateId,
        reason: String,
    },
}
```

Events are strategy opinions. The graph records them but does not treat them as universal truth.

### 15.1 Fitted preference state lives here

Stateful preference models (Bradley-Terry over pairwise judgments, Plackett-Luce over listwise rankings, fitted human-aggregators) are owned by `Population` impls — concretely, `TournamentPopulation` — rather than by `PreferenceRelation`. The reasoning:

- The state of the model depends on the run's accumulated observations.
- Updates fit naturally into `observe_assessment`, which the engine already calls when assessments land.
- `select_candidates` and `best` use the fitted model directly without crossing trait boundaries.
- `PreferenceRelation` stays stateless, simple, and `&self`-only.

```rust
pub struct TournamentPopulation<P: OptimizationProblem> {
    model: BradleyTerryFit,           // updated in observe_assessment
    candidates: BTreeSet<CandidateId>,
    config: TournamentConfig,
}

impl<P: OptimizationProblem> Population<P> for TournamentPopulation<P>
where
    P::Evidence: PairwiseEvidence,
{
    fn observe_assessment(
        &mut self,
        assessment: AssessmentId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        let a = graph.assessment(assessment);
        if let Assessment::Pairwise { left, right, evidence, .. } = a {
            self.model.update(left, right, evidence.judgment());
        }
        // …
    }

    fn best(&self, _graph: RunGraphView<'_, P>) -> Option<CandidateId> {
        self.model.argmax_score()
    }

    // …
}
```

### 15.2 ParetoFrontier and partition filtering

The standard `ParetoFrontier` population is built via a builder. Frontiers can declaratively ignore observations from specific case-set partitions. This is necessary for clean benchmark mode (only update from `SEARCH`, never from `TEST`) and for probe-eval handling.

```rust
let frontier = ParetoFrontier::<P, _>::builder()
    .axis_extracted("accuracy",       Direction::HigherIsBetter,
                    |e: &P::Evidence| e.accuracy())
    .axis_extracted("context_tokens", Direction::LowerIsBetter,
                    |e: &P::Evidence| e.context_tokens() as f64)
    .partition_filter(|target| matches!(target,
        AssessmentTarget::EvaluationSet(id) if is_search_partition(id)))
    .build();
```

Test-set assessments are still observed by the engine and recorded in the graph, but the frontier ignores them when deciding admission. Final test-set evaluation reads frontier members from outside the optimizer loop.

---

## 16. Trust and Capability Boundaries

Agentic stages require explicit boundaries.

### 16.1 Proposal context

```rust
pub struct ProposalContext<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
    renderers: &'a RendererRegistry<P>,
    budget: BudgetHandle<'a>,                     // unified budget access

    readable: ReadScope<P>,
    workspace: Option<WorkspaceFactory<P>>,
    eval: Option<EvalHandle<'a, P>>,
}
```

### 16.2 Eval handle

```rust
pub struct EvalHandle<'a, P: OptimizationProblem> {
    allowed_sets: EvaluationSetPermission,
    evidence_visibility: EvidenceVisibility,
    budget: BudgetHandle<'a>,                     // same type as ProposalContext
    recorder: ProbeRecorder<'a, P>,
}
```

### 16.3 Budget handle

The single type stages use to charge cost. Wraps the engine's `BudgetLedger` along with the stage tag, so charges are attributed automatically.

```rust
pub struct BudgetHandle<'a> {
    ledger: &'a mut BudgetLedger,
    stage: StageId,
}

impl<'a> BudgetHandle<'a> {
    pub fn snapshot(&self) -> BudgetSnapshot;
    pub fn remaining(&self, unit: CostUnit) -> Option<Amount>;

    /// Charge cost to the ledger under this handle's stage tag.
    pub fn charge(&mut self, cost: Cost) -> Result<(), BudgetExceeded>;

    /// Sub-stages (e.g. an evaluator's per-case subprocess invocation) get
    /// a re-borrowed handle with a more specific stage tag. The lifetime
    /// nests; only one mutable borrow exists at a time.
    pub fn sub_stage(&mut self, sub: StageId) -> BudgetHandle<'_>;
}
```

The point of the type is borrow safety: there is exactly one path from "I want to charge cost" to "the ledger gets mutated," and it's parameterized by a stage tag for free attribution. Stages never see `&mut BudgetLedger` directly.

`ctx.budget_handle()` returns `BudgetHandle` borrowed from the context's ledger. Callers can pass it into agent runtimes, evaluators, renderers, etc., and the borrow checker prevents two mutable accesses crossing.

Clean benchmark mode:

```text
proposer cannot read validation/test content
proposer cannot request validation/test probe evals
proposer sees only allowed evidence renderings
```

Exploratory mode:

```text
proposer may request probe evaluations
every probe is graph-recorded
probe candidates/assessments are tagged as probe-originated
population eligibility is controlled by policy
```

### 16.5 Workspace lifecycle

Agentic stages need a place to read and write files, possibly inside a sandbox. The library models this with three concepts that compose:

```
[engine]                  owns the WorkspaceFactory (chosen at config time)
   │
   ▼
[WorkspaceFactory]        creates Workspace handles on demand
   │                      (Local, E2B, Docker, K8s, Firecracker, GitWorktree, …)
   ▼
[Workspace]               a typed handle. filesystem ops + run-command.
   │                      backed by ONE sandbox of whatever flavor the factory makes.
   ├──▶ used by [WorkspaceRenderer]   (writes files into it)
   └──▶ used by [AgentRuntime]        (runs commands in it)
```

The workspace is the unit. One stage call (one `propose`, one `evaluate`) gets one workspace. It lives for the duration of that call and cleans up on drop.

#### 16.5.1 Trait surface

```rust
#[async_trait]
pub trait WorkspaceFactory: Send + Sync {
    async fn allocate(&self, cfg: WorkspaceConfig) -> Result<Workspace, FactoryError>;
}

pub struct Workspace {
    inner: Option<Box<dyn WorkspaceBackend>>,    // None after cleanup() consumes
}

impl Workspace {
    /// Explicit, awaited cleanup. Always preferred over relying on Drop.
    /// After this returns, the Workspace is consumed and its backend is gone.
    pub async fn cleanup(mut self) -> Result<(), WorkspaceError> {
        if let Some(backend) = self.inner.take() {
            backend.cleanup().await
        } else {
            Ok(())
        }
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        // Best-effort: synchronous local cleanup only. Cannot await async work
        // here, so remote sandbox destruction must go through cleanup().
        // If the backend was not consumed by an explicit cleanup() call, this
        // marks the workspace as "abandoned" — the factory's janitor (if any)
        // will reap it later.
        if let Some(backend) = self.inner.take() {
            backend.mark_abandoned();
        }
    }
}

#[async_trait]
pub trait WorkspaceBackend: Send + Sync {
    async fn write_file(&self, rel: &str, bytes: &[u8]) -> Result<()>;
    async fn read_file(&self, rel: &str) -> Result<Vec<u8>>;
    async fn run_command(&self, cmd: Command) -> Result<CommandOutput>;

    /// Async cleanup. Called by Workspace::cleanup. Implementors should fully
    /// release backend resources (destroy E2B sandbox, delete K8s container,
    /// remove git worktree) here.
    async fn cleanup(self: Box<Self>) -> Result<(), WorkspaceError>;

    /// Synchronous best-effort marking. Called by Workspace::Drop when the
    /// caller did not invoke cleanup(). Implementors should leave a marker
    /// for a factory-owned janitor to find later. Default: no-op (not all
    /// backends have a janitor; abandoned local tempdirs are usually fine).
    fn mark_abandoned(self: Box<Self>) {}

    /// For renderers that need a real local path (e.g. mounting into a subprocess
    /// running on the host). `None` for pure-remote backends like E2B without a
    /// local sync. Renderers should not depend on this returning `Some`.
    fn local_mount(&self) -> Option<&Path> { None }
}
```

**Always call `Workspace::cleanup().await` explicitly.** `Drop` is a safety net, not a primary cleanup path:

- Async work cannot be awaited inside `Drop`, so remote sandbox teardown (E2B, K8s, Firecracker) cannot happen there.
- A factory may run a periodic janitor that reaps abandoned workspaces — useful for crashes mid-run, but not a substitute for explicit cleanup.
- Stages with workspaces must call `cleanup()` on their workspace before returning, including in error paths. Use `?`-with-cleanup or a defer-style helper.

```rust
// idiomatic stage code with cleanup
async fn evaluate(&self, …) -> Result<…> {
    let mut ws = self.workspace_factory.allocate(…).await?;
    let outcome = self.run_evaluation(&mut ws).await;  // catch errors first
    ws.cleanup().await.ok();                            // always cleanup
    outcome                                             // then propagate
}
```

#### 16.5.2 Standard backends

The library ships a small set of reference backends. Most users will configure one of these or write their own:

```text
LocalWorkspaceFactory            tempdir on the host. cheap, no isolation. dev/test.
E2BWorkspaceFactory              one e2b sandbox per workspace. pooling supported.
DockerWorkspaceFactory           one docker container per workspace. local isolation.
K8sWorkspaceFactory              container-in-pod. pod is shared, container is per-workspace.
FirecrackerWorkspaceFactory      one microvm per workspace. strong isolation, slower spin-up.
GitWorktreeFactory<Inner>        wraps another factory; allocates a worktree at a parent commit.
```

`GitWorktreeFactory` is a composition: it takes any other factory as its inner sandbox and adds git-worktree semantics on top. The agent commits inside the worktree; the framework reads `HEAD` on cleanup; cleanup removes the worktree directory but leaves commit objects in the main repo. `content_id` of the resulting artifact is the commit hash.

#### 16.5.3 Ownership table

| Thing | Lifetime | Owner |
|---|---|---|
| `WorkspaceFactory` | Full run | Engine (configured at startup) |
| `Workspace` handle | One stage call | The stage that called `allocate` |
| Underlying sandbox/container/VM | Workspace handle's lifetime | `WorkspaceBackend` impl |
| Pooled warm sandboxes | Process-lifetime; idle-evicted | The factory |
| `AgentRuntime` instance | User-defined | The proposer (or a registry) |
| Files inside the workspace | Workspace handle | Wiped on cleanup |

#### 16.5.4 What the framework does NOT manage

- **Backend choice.** The factory is yours. The library does not assume e2b, docker, or any specific sandbox.
- **Agent processes.** That is the `AgentRuntime`'s job. The runtime uses the workspace as a substrate via `run_command`.
- **Pooling.** A factory may pool internally (recommended for slow cold-starts like e2b or firecracker). The framework does not pool by default.
- **Isolation guarantees.** Trust comes from the factory choice. A `LocalWorkspaceFactory` gives you no isolation; a `FirecrackerWorkspaceFactory` gives you strong isolation. Pick deliberately.

---

## 17. Cache

### 17.1 Engine-owned

The engine owns evaluation caching.

### 17.2 Cache key

```rust
pub struct EvaluationCacheKey {
    pub evaluator_fingerprint: Fingerprint,
    pub request_fingerprint: Fingerprint,
    pub candidate_content_ids: Vec<ContentId>,
    pub evaluation_set_id: EvaluationSetId,
    pub case_set_version: CaseSetVersion,
    pub seed: Option<u64>,
}
```

For pairwise requests, ordering is preserved unless evaluator declares unordered symmetry.

### 17.3 Cache policy

```rust
pub enum CachePolicy {
    Never,
    Deterministic,
    DeterministicWithSeed(u64),
    UserKey(Fingerprint),
}
```

Default is `Never`. Nondeterministic LLM/agent evaluators should not be cached accidentally.

---

## 18. Callbacks and Events

Callbacks are first-class.

```rust
pub trait Callback<P: OptimizationProblem>: Send {
    fn on_event(
        &mut self,
        event: &RunEvent<P>,
        graph: RunGraphView<'_, P>,
    );
}
```

Core events:

```rust
pub enum RunEvent<P: OptimizationProblem> {
    OptimizationStarted { run_id: RunId },

    OptimizationStopping { reason: StopReason },

    OptimizationEnded {
        run_id: RunId,
        best: Option<CandidateId>,
        budget: BudgetSnapshot,
    },

    IterationStarted { iteration: IterationId },

    IterationEnded {
        iteration: IterationId,
        status: StepStatus,
    },

    ProposalBatchProduced {
        iteration: IterationId,
        batch_id: ProposalBatchId,
        proposer: StageId,
        proposal_count: usize,
    },

    /// Per-proposal record. Effect kind and provenance summary live here;
    /// full causal_inputs / informed_by are queryable via graph.proposal_batch().
    ProposalRecorded {
        proposal_id: ProposalId,
        batch_id: ProposalBatchId,
        effect: ProposalEffectKind,         // Create or Change
        causal_inputs: CausalInputsSummary, // (variant + count)
        informed_by_count: usize,
    },

    ApplySucceeded {
        proposal_id: ProposalId,
        candidate_id: CandidateId,
        content_id: ContentId,
    },

    ApplyFailed {
        proposal_id: ProposalId,
        error: ErrorRecord,
    },

    EvaluationRequested {
        request_id: EvaluationRequestId,
        evaluator: EvaluatorId,
        request: EvaluationRequestSummary,
    },

    EvaluationCompleted {
        request_id: EvaluationRequestId,
        evaluator: EvaluatorId,
        assessment_ids: Vec<AssessmentId>,
        cost: Cost,
        cache: CacheStatus,
    },

    RenderCompleted {
        renderer: RendererId,
        target: String,
        cost: Cost,
    },

    PopulationUpdated {
        population_id: PopulationId,
        events: Vec<PopulationEvent>,
    },

    BudgetCharged {
        stage: StageId,
        cost: Cost,
        remaining: BudgetSnapshot,
    },

    CheckpointSaved {
        checkpoint: CheckpointId,
    },

    Error {
        stage: Option<StageId>,
        error: ErrorRecord,
        policy: ErrorPolicy,
    },
}
```

Engine emits lifecycle events. Context methods emit operation events.

---

## 19. Persistence and Evidence Storage

Core should not require:

```rust
Evidence: Serialize + DeserializeOwned
```

Instead:

```rust
pub trait EvidenceStore<E: Evidence>: Send + Sync {
    fn put(&self, evidence: E) -> Result<EvidenceRef, StoreError>;
    fn get(&self, reference: EvidenceRef) -> Result<E, StoreError>;
}
```

Default stores:

```text
InlineSerdeStore<E: Serialize + DeserializeOwned>
FileEvidenceStore
ObjectEvidenceStore
SqliteEvidenceStore
```

This avoids forcing giant agent traces into inline run graph serialization.

---

## 20. GEPA as an Optimizer

GEPA is one optimizer value.

```rust
pub struct Gepa<P, Prop, Pop, CandSel, CompSel, Batch, Gate, Val>
where
    P: OptimizationProblem,
{
    proposer: Prop,
    population: Pop,
    candidate_selector: CandSel,
    component_selector: CompSel,
    batch_sampler: Batch,
    gate: Gate,
    validation: Val,
    merge: Option<Box<dyn DynProposer<P>>>,
}
```

GEPA components:

```text
CandidateSelector selects candidate(s) from population.
ComponentSelector selects artifact component(s) to mutate.
BatchSampler selects train/minibatch cases.
Proposer emits proposal batch.
Gate decides whether a child gets validation.
ValidationPolicy decides validation request.
Population maintains Pareto/frontier/live set.
MergeScheduler decides when to call merge proposer.
```

Mapping to GEPA paper:

| GEPA algorithm concept | Library concept |
|---|---|
| candidate pool `P` | `Population` |
| Pareto front by instance | `ParetoFrontier::by_case()` |
| SELECTCANDIDATE | `CandidateSelector` |
| SELECTMODULE | `ComponentSelector` |
| minibatch from `D_feedback` | `BatchSampler` + `EvaluationSet::Partition(TRAIN)` |
| per-instance score table | `AssessmentGranularity::PerCase` |
| reflective prompt update | `Proposer` |
| score improves on minibatch | `Gate` |
| evaluate on `D_pareto` | `ValidationPolicy` |
| add to pool | `Population::observe_candidate` |
| merge/crossover | another `Proposer` scheduled by GEPA |

#### Merge canonicalization in GEPA

GEPA's merge picks per-component "best" parts from two candidates `(a, b)`. `Artifact::apply` only sees one artifact, so the merge proposer canonicalizes: it picks one parent (say `a`) as the apply target, reads `b` from the graph to extract the components it wants to import, and constructs a `Change` that — when applied to `a` — produces the merged content (e.g. `MultiComponentEdit { component_ids: […], replacement_contents: […from b…] }`). The resulting `Proposal` has `effect: ProposalEffect::Change { target: a, change }` and `provenance.causal: CausalInputs::Pair(a, b)` so lineage queries see both contributors, but the apply step is single-parent. The constructor sugar `Proposal::merge(a, b, change)` packages this.

GEPA customization:

```rust
let gepa = Gepa::default()
    .proposer(ReflectiveMutation::new(lm).n_alternatives(3))
    .population(ParetoFrontier::by_case().frequency_weighted())
    .component_selector(RoundRobin)
    .batch_sampler(EpochShuffled::new(4))
    .gate(StrictImprovement)
    .validation(FullValidation)
    .merge(SystemAwareMerge::adaptive());
```

---

## 21. MuF/Edit-Style Typed Claims

MuF/Edit fits as typed annotations.

```rust
pub struct EditAnnotations {
    pub diagnosis: MuFOutput,
    pub rationale: String,
    pub rhetorical_strategy: String,
    pub should_fix: String,
    pub should_not_break: String,
    pub rollback_note: String,
    pub confidence: Confidence,
}
```

Capability trait:

```rust
pub trait HasBehavioralClaims {
    fn should_fix(&self) -> &str;
    fn should_not_break(&self) -> &str;
    fn confidence(&self) -> Confidence;
}
```

Gate:

```rust
pub struct ClaimsHeldGate<J> {
    judge: J,
}

impl<P, J> Gate<P> for ClaimsHeldGate<J>
where
    P: OptimizationProblem<ProposalAnnotations = EditAnnotations>,
    J: ClaimJudge<P>,
{
    fn admit(
        &self,
        candidate: CandidateId,
        parent: CandidateId,
        scope: PreferenceScope,
        graph: RunGraphView<'_, P>,
    ) -> GateDecision {
        let proposal = graph.proposal_that_created(candidate);
        let claims = &proposal.annotations;

        if self.judge.claims_held(parent, candidate, claims, scope, graph) {
            GateDecision::Promote
        } else {
            GateDecision::RecordOnly
        }
    }
}
```

MuF/Edit is natural but not core-shaped. `should_fix` does not become a universal primitive.

---

## 22. User-Facing API Tiers

### 22.1 Tier 1: simple use

```rust
let result = optimize(seed_prompt)
    .cases(train_cases)
    .evaluate(|artifact, case| async move {
        // returns scalar evidence through an adapter
    })
    .using(Gepa::default().with_lm(lm))
    .budget(Budget::metric_calls(500))
    .run()
    .await?;
```

### 22.2 Tier 2: GEPA customization

```rust
let result = optimize(seed_agent)
    .cases(repo_tasks)
    .holdout(heldout_repo_tasks)
    .evaluate(RepoAgentEvaluator::new(sandbox))
    .using(
        Gepa::default()
            .proposer(AgenticProposer::new(runtime))
            .candidate_selector(ParetoFrequencyWeighted)
            .component_selector(WorstEvidenceComponent)
            .population(ParetoFrontier::by_case_and_axis())
            .proposal_count(3)
    )
    .budget(Budget::usd(100.0))
    .run()
    .await?;
```

### 22.3 Tier 3: optimizer author

```rust
struct MyTournamentOptimizer {
    /// Owns the fitted Bradley-Terry model internally.
    population: TournamentPopulation<MyProblem>,
}

impl Optimizer<MyProblem> for MyTournamentOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, MyProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let pair = self.population.select_candidates(Arity::Pair, ctx.graph().view());

        let report = ctx.evaluate_with(
            EvaluatorId::PAIRWISE_JUDGE,
            EvaluationRequest::Pairwise {
                left: pair[0],
                right: pair[1],
                set: EvaluationSet::Partition(PartitionId::TRAIN),
                granularity: AssessmentGranularity::Aggregate,
                purpose: EvaluationPurpose::Selection,
                order: PairOrder::Ordered,
            },
        ).await?;

        // observe_assessment updates the population's fitted Bradley-Terry model
        for assessment in report.assessments {
            let events = self.population.observe_assessment(assessment.id, ctx.graph().view());
            ctx.record_population_events(self.population.id(), events);
        }

        Ok(StepStatus::Continue)
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, MyProblem>,
    ) -> Option<CandidateId> {
        self.population.best(graph)
    }
}
```

### 22.4 Worked example: `gskill` — agentic evaluator with workspace materialization

`gskill` evolves a directory of skill files. The evaluator runs an LLM-based coding agent inside a sandboxed workspace against each task; the proposer is reflective. This exercises `Materializable` (the stdlib convenience trait), `WorkspaceFactory`, and the trust boundary in §16.

```rust
pub struct GskillProblem;
impl OptimizationProblem for GskillProblem {
    type Artifact = SkillDir;                    // a directory of .md skill files
    type Case = SweSmithTask;                    // task, repo, expected behavior
    type Evidence = ResolveEvidence;             // pass/fail + agent transcript ref
    type ProposalAnnotations = ReflectionNotes;
}

#[derive(Clone)]
pub struct SkillDir {
    files: BTreeMap<SkillFileId, Arc<str>>,
    content_id: ContentId,
}

impl Artifact for SkillDir {
    type Change = SkillEdit;                     // add/edit/remove a single file, or multi-edit
    type ApplyError = SkillError;
    fn content_id(&self) -> ContentId { self.content_id }
    fn apply(&self, c: &SkillEdit) -> Result<Self, _> { /* clone+mutate, rehash */ }
}

impl Decomposable for SkillDir {
    type ComponentId = SkillFileId;
    fn components(&self) -> Vec<Component<SkillFileId>> { /* one per file */ }
}

// stdlib convenience trait — a SkillDir has an obvious canonical layout.
// stdlib WorkspaceRenderer impls use this when present.
impl Materializable for SkillDir {
    async fn materialize(
        &self,
        ws: &mut WorkspaceView<'_>,
    ) -> Result<RenderReport, MaterializeError> {
        let mut count = 0;
        for (id, content) in &self.files {
            ws.write_file(&format!("skills/{}.md", id), content.as_bytes()).await?;
            count += 1;
        }
        Ok(RenderReport::file_count(count))
    }
}

pub struct GskillEvaluator<R: AgentRuntime> {
    workspace_factory: Arc<dyn WorkspaceFactory>,
    runtime: R,
    cases: Arc<CaseSet<SweSmithTask>>,
    evidence_store: Arc<dyn EvidenceStore<ResolveEvidence>>,
}

#[async_trait]
impl Evaluator<GskillProblem> for GskillEvaluator<MiniSweAgentRuntime> {
    async fn evaluate(
        &self,
        request: EvaluationRequest,
        mut ctx: EvaluationContext<'_, GskillProblem>,
    ) -> Result<Metered<Vec<Assessment<GskillProblem>>>, EvaluationError> {
        let EvaluationRequest::Independent { candidates, set, .. } = request
            else { return Err(EvaluationError::UnsupportedRequestShape); };

        let mut out = Vec::new();
        let mut total_cost = Cost::zero();

        for cand in candidates {
            let artifact = ctx.graph().artifact(cand).unwrap();
            let mut per_case = BTreeMap::new();

            for case_id in set.resolve(&self.cases)? {
                // fresh workspace per case (sandboxing matters here — the agent runs
                // arbitrary tool calls against the task's repo)
                let mut ws = self.workspace_factory.allocate(WorkspaceConfig::default()).await?;
                artifact.materialize(&mut ws.view()).await?;

                let case = self.cases.get(case_id);
                let session = self.runtime.run_session(&ws, AgentSessionConfig {
                    task: case.task_description.clone(),
                    repo: case.repo_url.clone(),
                    skills_path: "/workspace/skills".into(),
                    budget: ctx.budget_handle(),
                }).await?;

                total_cost = total_cost + session.cost;
                let trace_ref = self.evidence_store.put_blob(&session.transcript).await?;

                per_case.insert(case_id, ResolveCaseResult {
                    resolved: session.resolved,
                    trace_ref,
                });
                ws.cleanup().await?;
            }

            out.push(Assessment::Independent {
                candidate: cand,
                target: AssessmentTarget::EvaluationSet(set.id()),
                evidence: ResolveEvidence { per_case },
                cost: total_cost.clone(),
                metadata: MetadataBag::new(),
            });
        }

        Ok(Metered::new(out, total_cost))
    }

    fn cache_policy(&self, _: &EvaluationRequest) -> CachePolicy { CachePolicy::Never }
    fn fingerprint(&self) -> Fingerprint { /* runtime + cases versions */ }
    fn id(&self) -> EvaluatorId { EvaluatorId::PRIMARY }
}
```

Driver code:

```rust
let result = optimize(seed_skills)
    .cases(swe_smith_tasks)
    .partitions(&[(PartitionId::TRAIN, train_ids), (PartitionId::VALIDATION, val_ids)])
    .evaluator(GskillEvaluator {
        workspace_factory: Arc::new(E2BFactory::pooled(/* … */)),
        runtime: MiniSweAgentRuntime::new(/* model config */),
        cases: Arc::new(swe_smith_tasks.clone()),
        evidence_store: Arc::new(SqliteEvidenceStore::open("traces.db")?),
    })
    .using(
        Gepa::default()
            .proposer(ReflectiveMutation::with_lm(reflection_lm))
            .component_selector(WorstEvidenceComponent)
            .population(ParetoFrontier::by_case())
    )
    .trust_policy(TrustPolicy::HideFromProposer(&[PartitionId::VALIDATION]))
    .budget(Budget::usd(50.0))
    .run()
    .await?;
```

Key takeaways:

- `Materializable` (stdlib convenience) is the bridge from typed artifact to filesystem layout the agent reads. For artifacts without an obvious canonical layout, write a `WorkspaceRenderer` directly.
- `WorkspaceFactory` (here e2b, pooled) handles sandbox topology; the evaluator uses `&Workspace` agnostically.
- One workspace per case is the evaluator's choice — for skill evolution where the agent mutates the repo, isolation matters.
- `EvidenceStore` keeps multi-MB agent transcripts out of the inline graph.
- `TrustPolicy::HideFromProposer` ensures the reflective proposer never sees validation case content.

### 22.5 Worked example: Meta-Harness — agentic proposer over full graph history

Meta-Harness (Lee et al. 2026) writes a fresh harness program each iteration, with a coding-agent proposer that reads the entire run history through a filesystem. This exercises `WorkspaceRenderer`, `ProposalEffect::Create`, `Arity::None`, multi-axis `ParetoFrontier`, and the rendering of large execution traces.

```rust
pub struct MetaHarness;
impl OptimizationProblem for MetaHarness {
    type Artifact = HarnessArtifact;             // single .py file
    type Case = ClassificationCase;
    type Evidence = HarnessEvidence;             // per-case correctness + token cost
    type ProposalAnnotations = ProposerNotes;
}

// Artifact, Evaluator — same pattern as gskill, omitted for brevity.
// HarnessArtifact has a stdlib Materializable impl that writes harness.py.

// The history renderer is the load-bearing piece. It populates a workspace
// with per-candidate directories the agent will grep.
pub struct MetaHarnessHistoryRenderer<AR, TR> {
    artifact_renderer:  AR,    // WorkspaceRenderer<MetaHarness, HarnessArtifact>
    traces_renderer:    TR,    // WorkspaceRenderer<MetaHarness, HarnessEvidence>
    task_description:   Arc<str>,
    instructions:       Arc<str>,
}

#[async_trait]
impl<AR, TR> WorkspaceRenderer<MetaHarness, HistorySnapshot<'_>>
    for MetaHarnessHistoryRenderer<AR, TR>
where
    AR: WorkspaceRenderer<MetaHarness, HarnessArtifact>,
    TR: WorkspaceRenderer<MetaHarness, HarnessEvidence>,
{
    async fn render_into(
        &self,
        snap: &HistorySnapshot<'_>,
        ws: &mut WorkspaceView<'_>,
        ctx: RenderContext<'_, MetaHarness>,
    ) -> Result<Metered<RenderReport>, RenderError> {
        ws.write_file("README.md", self.instructions.as_bytes())?;
        ws.write_file("TASK.md",   self.task_description.as_bytes())?;

        let mut harnesses = ws.subdir("harnesses")?;
        for &cand in &snap.visible_candidates {
            let cand_view = snap.graph.candidate(cand).unwrap();
            let mut dir = harnesses.subdir(&directory_name_for(cand, &snap.graph))?;

            self.artifact_renderer.render_into(&cand_view.artifact, &mut dir, ctx.clone()).await?;
            dir.write_file("scores.json", scores_summary_json(&snap.graph, cand).as_bytes())?;

            let mut traces = dir.subdir("traces")?;
            for assessment in snap.graph.assessments(cand) {
                // ReadScope hides test-partition assessments; renderer respects it
                if !is_visible(&assessment.target, &ctx.read_scope()) { continue; }
                self.traces_renderer.render_into(&assessment.evidence, &mut traces, ctx.clone()).await?;
            }
        }
        let _ = ws.subdir("output")?;     // where the agent writes new harnesses
        Ok(Metered::new(RenderReport::default(), Cost::zero()))
    }
}

// The proposer's request type — owned and lightweight. The proposer builds
// the borrowed HistorySnapshot internally from ctx.graph() inside propose().
// This avoids putting a lifetime on the trait's associated type.
pub struct HistoryProposalRequest {
    /// Which candidates the optimizer wants the agent to consider.
    /// Computed by the optimizer from its read scope; the proposer doesn't
    /// re-derive visibility.
    pub visible_candidates: Vec<CandidateId>,
    pub k: usize,
}

// The agentic proposer. arity = None: no causal parents.
// Note the renderer is a stage-owned field, not a registry lookup.
pub struct AgenticHarnessProposer<R, HR> {
    runtime: R,                                            // claude-code wrapper
    history_renderer: HR,
}

#[async_trait]
impl<R, HR> Proposer<MetaHarness> for AgenticHarnessProposer<R, HR>
where
    R: AgentRuntime,
    // The renderer takes any HistorySnapshot lifetime; we'll feed it a borrow
    // of the graph view we hold inside propose().
    HR: for<'a> WorkspaceRenderer<MetaHarness, HistorySnapshot<'a>>,
{
    type Request = HistoryProposalRequest;

    fn id(&self) -> ProposerId { ProposerId::new("meta_harness/claude_code") }
    fn arity(&self) -> Arity { Arity::None }

    async fn propose(
        &self,
        request: Self::Request,
        mut ctx: ProposalContext<'_, MetaHarness>,
    ) -> Result<Metered<ProposalBatch<MetaHarness>>, ProposalError> {
        // Build the borrowed snapshot from ctx.graph(). It lives only for this
        // call; the renderer consumes it before the await on run_session.
        let snapshot = HistorySnapshot {
            graph: ctx.graph(),
            visible_candidates: &request.visible_candidates,
            current_iteration: ctx.current_iteration(),
        };

        let mut ws = ctx.workspace.as_ref().unwrap().allocate(WorkspaceConfig::default()).await?;
        let render = self.history_renderer.render_into(
            &snapshot, &mut ws, ctx.render_context()
        ).await?;

        let session = self.runtime.run_session(&ws, AgentSessionConfig {
            task: HARNESS_SEARCH_PROMPT,
            output_dir: ws.path().join("output"),
            budget: ctx.budget_handle(),
        }).await?;

        let referenced = parse_referenced_candidates(&session.transcript);

        let mut proposals = Vec::new();
        for i in 0..request.k {
            let path = ws.path().join(format!("output/harness_{i}.py"));
            let Ok(source) = tokio::fs::read_to_string(&path).await else { continue };
            let notes = read_optional(&ws, &format!("output/notes_{i}.md")).await;

            // ProposalEffect::Create — brand-new authored artifact.
            // No "Change applied to nothing" lie; the proposal honestly says
            // "create this artifact, here's what informed me."
            proposals.push(
                Proposal::create(HarnessArtifact::from_source(Arc::from(source)))
                    .informed_by(referenced.iter().map(|&c| InfoRef::Candidate(c)))
                    .annotations(ProposerNotes { rationale: notes })
                    .build()
            );
        }

        // Workspace cleanup is explicit (Drop is best-effort only; see §16.5).
        ws.cleanup().await.ok();

        Ok(Metered::new(
            ProposalBatch {
                proposals,
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            render.cost + session.cost,
        ))
    }
}

// Optimizer: no candidate selector, no merge, just propose-evaluate-observe.
pub struct MetaHarnessOptimizer<R, HR, Axes> {
    proposer:    AgenticHarnessProposer<R, HR>,
    population:  ParetoFrontier<MetaHarness, Axes>,
    k_per_iter:  usize,
}

#[async_trait]
impl<R, HR, Axes> Optimizer<MetaHarness> for MetaHarnessOptimizer<R, HR, Axes>
where
    R: AgentRuntime,
    HR: WorkspaceRenderer<MetaHarness, HistorySnapshot<'static>>,
    Axes: ParetoAxes<MetaHarness>,
{
    async fn step(&mut self, ctx: &mut RunContext<'_, MetaHarness>)
        -> Result<StepStatus, OptimizerError>
    {
        // Compute which candidates are visible to the proposer right now.
        // This is owned data; no graph references in the request itself.
        let visible_candidates = ctx
            .graph()
            .candidates_visible_to(ctx.read_scope())
            .collect();

        let report = ctx.propose(
            &self.proposer,
            HistoryProposalRequest {
                visible_candidates,
                k: self.k_per_iter,
            },
        ).await?;

        // apply_batch processes every proposal; interface validation lives in
        // HarnessArtifact::apply (or the artifact constructor for Create);
        // failures land as ApplyFailed events in the graph.
        let applied = ctx.apply_batch(report.batch).await?;

        for cand_id in applied.successful_candidates() {
            let eval = ctx.evaluate(EvaluationRequest::Independent {
                candidates: vec![cand_id],
                set: EvaluationSet::Partition(PartitionId::SEARCH),
                granularity: AssessmentGranularity::PerCase,
                purpose: EvaluationPurpose::Search,
            }).await?;
            for a in eval.assessments {
                let events = self.population.observe_assessment(a.id, ctx.graph().view());
                ctx.record_population_events(self.population.id(), events);
            }
        }
        Ok(StepStatus::Continue)
    }

    fn best_candidate(&self, g: RunGraphView<'_, MetaHarness>) -> Option<CandidateId> {
        self.population.best(g)
    }
}
```

Driver code:

```rust
let frontier = ParetoFrontier::<MetaHarness, _>::builder()
    .axis_extracted("accuracy",       Direction::HigherIsBetter,
                    |e: &HarnessEvidence| e.accuracy())
    .axis_extracted("context_tokens", Direction::LowerIsBetter,
                    |e: &HarnessEvidence| e.context_tokens() as f64)
    .partition_filter(|t| matches!(t,
        AssessmentTarget::EvaluationSet(id) if is_search_partition(id)))
    .build();

let result = optimize(seed_harnesses)
    .cases(text_classification_tasks)
    .partitions(&[(PartitionId::SEARCH, search_ids), (PartitionId::TEST, test_ids)])
    .evaluator(HarnessEvaluator { /* … */ })
    .using(MetaHarnessOptimizer {
        proposer: AgenticHarnessProposer { /* claude code in firecracker */ },
        population: frontier,
        history_renderer: history_renderer.clone(),
        k_per_iter: 2,
    })
    .trust_policy(TrustPolicy::HideFromProposer(&[PartitionId::TEST]))
    .budget(Budget::new().iterations(20).usd(500.0))
    .run()
    .await?;
```

Key takeaways:

- **`ProposalEffect::Create` and `Arity::None`** are essential for this style: the agent authors fresh harnesses each iteration; the proposal is honestly a `Create`, not a `Change` whose target is meaningless. Lineage is bibliographic via `informed_by`, never causal.
- **`WorkspaceRenderer`** is the load-bearing primitive. The orchestrator renderer composes per-artifact and per-evidence sub-renderers into the candidate-per-directory layout the agent greps.
- **`ParetoFrontier::partition_filter`** keeps the test partition out of the frontier even though it's still observable to post-run evaluation.
- **`TrustPolicy::HideFromProposer`** combines with the renderer's `read_scope` check to ensure test-partition traces never appear in the agent's workspace.
- **`EvidenceStore`** is non-optional at this scale — execution traces can hit 10M tokens; only refs live in the graph.

---

## 23. Standard Library Surface

### 23.1 Artifacts

```text
TextArtifact
PartMapArtifact
DirArtifact
GitArtifact
#[derive(Optimize)]
```

### 23.2 Evidence

```text
ScalarEvidence
ScoreVectorEvidence
PairwiseJudgmentEvidence
ListwiseRankingEvidence
MixedEvidence
StringEvidence
JsonEvidence
```

### 23.3 Preference relations

```text
HigherScoreIsBetter
LowerScoreIsBetter
ParetoPreference
LexicographicPreference
CopelandPreference
BordaPreference
CondorcetPreference
```

Note: `BradleyTerryFit` and `PlackettLuceFit` are *fitted models*, not stateless preference relations. They live on `TournamentPopulation<F>` (see §15.1 and §23.4).

### 23.4 Populations

```text
KeepBest
ParetoFrontier
MapElites
BeamPopulation
IslandsPopulation
TournamentPopulation
LenientParetoFrontier
NoveltyPopulation
NoPopulation
```

### 23.5 GEPA pieces

```text
ReflectiveMutation
SystemAwareMerge
ParetoFrequencyWeighted
RoundRobinComponent
WorstEvidenceComponent
EpochShuffled
StrictImprovement
ImprovementOrEqual
NoRegression
FullValidation
MinibatchThenValidation
```

### 23.6 Stages

```text
EvaluatorFn
ProposerFn
LmProposer
AgenticProposer
DiagnoseAndPropose
SurrogateProposer
EnsembleProposer
```

---

## 24. Trait Laws

### Artifact

```text
apply is functional.
failed apply does not mutate artifact state.

content_id is a deterministic hash that satisfies:
  same observationally-equivalent content => same id, with collision
  probability negligible at the run's scale.

the cache trusts content_id absolutely. lying about it produces
  silently incorrect cache results.

contract on the user (not framework-enforced):
  - artifacts are observationally immutable; no interior mutability
    that affects library-visible behavior.
  - content_id encodes everything an evaluator/renderer/change
    might depend on for this run's configuration.
  - hash is deterministic across machines (canonicalize maps, sets,
    floats, unicode).

content-addressed external handles satisfy these trivially:
  - git commit hash IS a hash of the underlying tree
  - IPFS CID IS a hash of the content
  - docker image digest IS a hash of the layers
  use these as content_id directly; no further hashing needed.

safety:
  - prefer #[derive(Optimize)]; it generates safe-by-default field
    hashing with explicit opt-out via #[content_skip].
  - in dev mode, set verify_cache_consistency = true to catch contract
    violations by re-evaluating on cache hits and comparing results.

hash strength:
  - blake3 or sha-256 recommended for any cross-run / cross-machine
    use (durable cache, content-addressed evidence storage).
  - 128-bit non-cryptographic hashes (xxh3-128) acceptable for
    in-process caching only.
  - 64-bit hashes are unsafe at typical run scales (>10^5 candidates).
```

### Decomposable

```text
component IDs are stable unless a change explicitly removes/replaces that component.
if identity is path-based, rename is remove + add.
if rename continuity matters, artifact must encode stable IDs.
```

### Evaluator

```text
returns assessments matching request shape.
reports all costs.
declares cache policy honestly.
must not mutate artifact state in graph.
must return UnsupportedGranularity when it cannot provide requested granularity.
```

### PreferenceRelation

```text
may return Incomparable.
must document whether it is total, partial, stochastic, fitted, or graph-derived.
must not silently treat missing evidence as zero unless explicitly documented.
```

### Population

```text
select_candidates returns existing candidates.
best may return None.
population events are strategy opinions, not graph truth.
population must not erase graph history.
```

### Optimizer

```text
all graph mutations happen through RunContext.
all costful work happens through metered stages or explicit budget charges.
may stop itself, but external stoppers remain engine-owned.
```

### Proposal (effect / provenance validation)

The framework validates these combinations before recording a proposal in the
graph. Invalid combinations return `ApplyError::InvalidProposal` and are
recorded as `ApplyFailed` events. They are NOT silent passes.

```text
ProposalEffect::Create  + CausalInputs::None         OK  (fresh authoring; Meta-Harness)
ProposalEffect::Create  + CausalInputs::NAry(...)    OK  (aggregate of N -> 1)
ProposalEffect::Create  + CausalInputs::Single(_)    INVALID
                                                     (use Change with that target instead)
ProposalEffect::Create  + CausalInputs::Pair(_, _)   INVALID
                                                     (Pair only meaningful for merge under Change)

ProposalEffect::Change  + CausalInputs::Single(p)    OK iff target == p
                                                     (otherwise: which is the apply target?)
ProposalEffect::Change  + CausalInputs::Pair(a, b)   OK iff target == a OR target == b
                                                     (target is the canonical apply parent;
                                                      change embeds content from the other)
ProposalEffect::Change  + CausalInputs::None         INVALID
                                                     (cannot apply to nothing)
ProposalEffect::Change  + CausalInputs::NAry(...)    OK iff target ∈ NAry list
                                                     (rare; for n-ary structured merges)
```

informed_by has no validation constraints — it's a free-form bibliography of
candidates, assessments, proposals, or external references the proposer read.
Empty informed_by is fine. Self-referential informed_by (where the proposer
records reading evidence about a candidate it later created) is also fine
because content_id determines whether they're the same.

### Renderer / WorkspaceRenderer

```text
rendering is a view, not a transformation of truth.
lossy rendering must be explicit.
target (or workspace contents) determines rendering shape.
costful rendering reports cost via Metered.
WorkspaceRenderers must respect the caller's read_scope:
  do not write evidence from forbidden partitions into the workspace.
WorkspaceRenderers should be idempotent within a single workspace
  (calling the same renderer twice with the same value is a no-op or
  produces the same files).
```

---

## 25. Expressibility Targets

The design must express naturally:

```text
GEPA
GEPA+Merge
MIPRO / MIPROv2
TextGrad
Trace / OptoPrime
MuF/Edit
MAP-Elites for prompts
C-Evolve
MOPrompt
GSkill
MemSkill
SkillFoundry
EvoSkills
Graph-of-Skills
Memento-Skills
VISTA
TEP
Pareto-lenient consensus
AlphaEvolve / OpenEvolve / ShinkaEvolve
pairwise-tournament continual learning
single-task keep-best search
recursive meta-optimization
ComBE-style aggregation
confidence-aware logprob-derived evaluators
```

Pass condition: a competent model can implement the optimizer using user-facing primitives without new core traits or engine modifications.

---

## 26. Implementation Plan

The prototypes are deliberately ordered to surface design problems early. P2 stresses what is *new* in this design (Pairwise eval requests, fitted preference relations, tournament populations); P3 validates that the design *also* expresses the well-understood case (GEPA). If P2 fights the API, you learn at the cheapest possible moment. If P3 lands clean, GEPA parity is a refinement exercise rather than a validation step.

### Prototype 1: scalar keep-best single-task

Goal: prove `Optimizer + RunContext + RunGraph`.

```text
TextArtifact
scalar evidence
HigherScoreIsBetter
KeepBest
simple mutation proposer
no dataset or singleton dataset
```

### Prototype 2: pairwise tournament

Goal: stress the parts of this design that don't exist in Python GEPA — pairwise evaluation requests, fitted preference relations on populations, tournament-style step rhythms.

```text
pairwise LLM judge evaluator
PairwiseJudgmentEvidence
TournamentPopulation (owns its Bradley-Terry fit)
EvaluationRequest::Pairwise
EvaluatorRegistry with EvaluatorId::PAIRWISE_JUDGE
```

### Prototype 3: GEPA parity

Goal: reproduce Python GEPA shape naturally on top of the surface validated by P1 and P2.

```text
PartMapArtifact
ReflectiveMutation
ProposalBatch::Alternatives
AssessmentGranularity::PerCase
ParetoFrontier::by_case
ParetoFrequencyWeighted
RoundRobinComponent
StrictImprovement
train/validation partitions
```

### Prototype 4: agentic Git artifact

Goal: prove rendering, materialization, trust boundaries, and the workspace lifecycle.

```text
GitArtifact + GitWorktreeFactory
WorkspaceRenderer composition (artifact + traces + history orchestrator)
agentic proposer with ProposalEffect::Create
repo-task evaluator with isolated workspaces
AgentTrajectoryEvidence via EvidenceStore
budget and sandbox hooks
```

---

## 27. Open Questions

### 27.1 Renderer registry typing — RESOLVED

Resolved by splitting rendering into two trait families: `Renderer<P, T, Target>` (value-returning) and `WorkspaceRenderer<P, T>` (side-effecting workspace population). See §13. The value-returning case can use a typed registry keyed by `(T, Target, View)`; the workspace case has no `View` ambiguity because the side effect *is* the output. Common renderers are still typically fields on stages (composition over registry); the registry is for cross-stage shared rendering and debug/inspection paths.

### 27.2 Evidence persistence

Core should not require serde on `Evidence`. Default stores support serde evidence; large evidence uses external stores. See §19.

### 27.3 Optimizer dyn dispatch

Do we need `Box<dyn DynOptimizer<P>>`? Probably not for v0.1. Optimizers are static values. Revisit if runtime-loaded optimizers become necessary.

### 27.4 Distributed execution

Out of scope for v0.1. Graph/event design should not preclude future merging.

### 27.5 Cache correctness for stochastic evaluators

Default no-cache. Deterministic cache only with explicit evaluator fingerprint and cache policy.

### 27.6 Preference relation state — RESOLVED

Resolved by placing fitted/stateful preference models on `Population` impls (concretely `TournamentPopulation`) rather than on `PreferenceRelation`. The state of a fitted model depends on accumulated observations; updates fit naturally into `observe_assessment`; `select_candidates` and `best` use the fit directly. `PreferenceRelation` stays stateless and `&self`-only. See §14 and §15.1.

### 27.7 Renderer registry vs stage-owned composition

Surfaced by the Meta-Harness walkthrough: a complex `WorkspaceRenderer` (e.g. `MetaHarnessHistoryRenderer`) is a composition of smaller renderers (`ArtifactRenderer`, `TracesRenderer`). Today these compose by holding `Arc<dyn WorkspaceRenderer<…>>` fields. A typed registry could replace these fields with lookups, but the field-based composition is more explicit and easier to typecheck. Recommend deferring a registry until a real second user wants different sub-renderers without recompiling.

---

## 28. Non-goals

```text
Python GEPA API compatibility
CLI
hosted service
distributed engine
built-in observability backend
specific LLM SDK dependency
automatic artifact-structure inference
automatic evidence-shape inference
skill marketplace
domain-specific shortcuts in core
```

---

## 29. Final Design Thesis

The library should make this sentence true:

> A Rust optimizer is a configured value that drives a typed run graph by proposing changes to artifacts, requesting assessments, interpreting evidence through preference relations, and maintaining live populations, while the engine provides budgeted, observable, capability-scoped execution.

Everything else falls out.

GEPA is one optimizer. MIPRO is one optimizer. TextGrad is one optimizer. A future paper should be one optimizer.

The engine is dumb. The optimizer is smart. The types tell the truth.

---

## 30. Changelog

### v0.2.1a (2026-05-06) — pre-implementation patch

Project name locked: **leaven**. A pre-implementation review of v0.2.1 flagged real Rust-mechanics issues (lifetime-on-trait, async-Drop, scattered `&mut BudgetLedger`) and residual wording inconsistencies from the v0.2 → v0.2.1 edit pass. v0.2.1a is the last polish before P0/P1 prototypes.

#### Type-level fixes

- **`Proposer::Request` no longer requires `'static`.** Convention spelled out: requests are owned and lightweight (just identify *what to do*); proposers construct rich views internally from `ctx.graph()`. The Meta-Harness example was updated to construct its `HistorySnapshot` inside `propose`, not pass it through the trait's associated type. Removes lifetime gymnastics on the trait.
- **`<P::Artifact as Artifact>::Change` is the canonical change type.** `OptimizationProblem` does not define `type Change`. Constructor sugar signatures fixed throughout.

#### New explicit machinery

- **§8.3 Report types defined explicitly.** `ProposalBatchReport`, `ApplyReport`, `ApplyOneReport`, `EvaluationReport`. Reports return IDs and graph-backed views, not graph-owned values. Includes `ApplyOutcome::{Success, Failure}` for per-proposal outcome tracking.
- **§5.9 `ResolvedEvaluationRequest` and resolution boundary.** RunContext resolves dynamic sets (`Recent`, `Sample`, `Stratified`) before passing to evaluators; cache key uses `ResolvedEvaluationSetId`. Evaluators never see unresolved expressions. Both expressions are recorded in the graph.
- **§16.3 `BudgetHandle<'a>` is the single budget access type.** Replaces scattered `&'a mut BudgetLedger` references on `ProposalContext` and `EvalHandle`. Wraps ledger + stage tag; one mutable borrow path; `sub_stage()` for nested attribution. Prevents borrow-hostile multi-handle situations.

#### Lifecycle clarifications

- **`Workspace::cleanup(self)` is explicit, not Drop-driven.** Async cleanup cannot be reliably awaited in `Drop`. The trait now distinguishes `async fn cleanup` (full backend teardown) from `Drop::mark_abandoned` (sync best-effort marker for factory janitors). Stages must call `cleanup().await` explicitly; idiomatic pattern documented.
- **§24 Proposal validation laws.** Per-combination rules for `(ProposalEffect, CausalInputs)`. Cheap correctness checks before graph insertion. Invalid combinations produce `ApplyFailed` events, never silent passes.

#### Wording cleanup

- **`informed_by` consistently described as `ProposalProvenance::informed_by`.** §0.1 entry 17 and §10.2 doc comment fixed; "backed by typed metadata" wording removed.
- **`BradleyTerryPreference` removed from stateless-preference lists.** Renamed to `BradleyTerryFit` (model object), placed under populations as `TournamentPopulation<BradleyTerryFit>`. §3 nomenclature, §5.15, §14, and §23.3 updated. `CopelandPreference` and `BordaPreference` (stateless graph aggregation) remain where they were.

#### Branding

- Project named **leaven**. Tagline: *Optimize anything in Rust.* Crate plan: umbrella `leaven` re-exporting `leaven-core`, `leaven-engine`, `leaven-std`, `leaven-workspace`, `leaven-derive`. Metaphor matches the design's "set up conditions, walk away, come back to a transformed substrate" pattern.

#### Stress tests still pass

The four pressure tests from v0.2 (cross-branch synthesis, Meta-Harness, workspace lifecycle, multi-agent composite) all still pass against v0.2.1a. The §22.4 (gskill) and §22.5 (Meta-Harness) worked examples have been updated for the new types and the cleanup pattern.

### v0.2.1 (2026-05-06) — post-review tightening

External review of v0.2 (sharp, terse, mostly fair) flagged that v0.2 retained shapes from v0.1 that became lies once new capabilities were layered in. Specifically: a `Proposal` carrying `parents: Parents::None + change` was incoherent for fresh-author cases like Meta-Harness; `informed_by` was promised as "a typed graph relation" but backed by stringly-typed metadata; the universal `ProposalRequest<P>` would collapse to an enum or bag once multiple proposer shapes shipped. v0.2.1 fixes those without changing architecture.

#### Proposal model

- **`Proposal::effect: ProposalEffect`** replaces bare `change + parents` (§5.5). `ProposalEffect::Create { artifact }` for fresh authoring; `ProposalEffect::Change { target, change }` for mutation. Removes the "Change applied to nothing" lie that `Parents::None` produced.
- **`Proposal::provenance: ProposalProvenance`** replaces inline `parents` and stringly-typed informed_by (§5.5). `causal: CausalInputs` records lineage that contributed to `content_id`; `informed_by: Vec<InfoRef>` records bibliographic influence that did not. Both are typed structured fields.
- **Constructor sugar** (`Proposal::mutate / merge / create / aggregate` + `ProposalBuilder`) keeps common cases one-line. Verbosity tax paid by the spec, not by users.
- **Merge canonicalization** documented inline: `Proposal::merge(a, b, change)` produces `effect: Change { target: a, change }` with `causal: Pair(a, b)`. The change embeds content sourced from `b`.

#### Proposer shape

- **`Proposer::Request` is an associated type** (§12). GEPA reflective mutation, merge, Meta-Harness, ComBE, MIPRO acquisition, and human edits don't share a request shape; an associated type is the rust-native answer and matches the static-first proposer story already chosen in v0.1. `DynProposer` wraps the request as `Box<dyn Any>` for runtime-loaded plugins.
- **`Arity` reframed as a request hint** (§12). Describes what shape the optimizer should provide as input *when the optimizer drives parent selection*. Proposers may emit proposals with different causal shapes than declared arity.

#### Context shape

- **`RunContext::apply_batch` and `apply_proposal`** replace `apply(parents, batch)` (§8.2). Per-proposal effects subsume the parents argument.

#### Removed

- **`Parents` enum.** Subsumed by `CausalInputs` (variant names match) plus `ProposalEffect` (which captures the apply target, not the parent).
- **`ProposalBatchSemantics::Ordered`** (§5.7). Multi-batch optimizer rhythm covers ordered-dependency cases. Re-add if a real prototype forces it.
- **`Materializable` from cold core** (§5.1). Moved to standard library as a convenience trait used by default `WorkspaceRenderer` impls. Cold-core `Artifact` stays free of workspace concerns.

#### Renderer policy

- **Stage-owned renderers are the default** (§13.4). Most stages should hold renderers as fields (`pub renderer: R`). `RendererRegistry` exists for cross-stage shared rendering and debug, not as the central path.

#### Trait law softening

- **`ContentId` law no longer says "MUST be a cryptographic hash of all observationally-relevant state"** (§24). Reframed as: "deterministic hash, same content => same id, collision probability negligible at run scale, the cache trusts it, lying produces silently wrong cache results." Content-addressed external handles (git commit hashes, IPFS CIDs, docker digests) trivially satisfy this. Contract on the user; framework not enforcing. Dev-mode `verify_cache_consistency` catches violations by re-evaluating on cache hits. No `Artifact / ContentAddressed` trait split — premature option-creation for use cases that haven't appeared.

#### Event shapes refreshed

- `ProposalBatchProduced` no longer carries `parent_ids` (§18). New `ProposalRecorded` event carries per-proposal effect kind, causal-inputs summary, and informed_by count. Full provenance via `graph.proposal_batch(id)`.

#### Open questions

- **27.7 Renderer registry vs stage-owned composition** is now answered: stage-owned by default, registry for plugin/debug. Removed from open questions.

#### Stress tests re-run

The four pressure tests from v0.2 (cross-branch synthesis, Meta-Harness, workspace lifecycle, multi-agent composite) still pass against v0.2.1 with cleaner code. Worked examples in §22.4 (gskill) and §22.5 (Meta-Harness) updated to use the new types.

### v0.2 (2026-05-06) — post-stress-test refinement

The v0.1 second-pass spec survived the conceptual stress tests. The corrections in this pass are local refinements that emerged when implementations were walked through end-to-end against four pressure tests: cross-branch synthesis, the Meta-Harness paper, the workspace-abstraction case, and the multi-agent-system case.

#### Type-level changes

- **`parents` moved from `ProposalBatch` to `Proposal` (§5.5, §5.7).** Sibling proposals in one batch can have different causal parents (cross-branch synthesis surfaced this). The batch carries `semantics + metadata`; each proposal carries its own `parents`.
- **`Parents::None` added (§5.5).** Brand-new authored artifacts with no causal predecessor — the Meta-Harness pattern. Lineage is bibliographic via `informed_by`, not causal.
- **`Arity::None` added (§12).** Proposers that don't ask for parents at all.

#### Trait surface changes

- **Renderer split into two trait families (§13).**
  - `Renderer<P, T, Target>` — value-returning, for prompt assembly, JSON, debug HTML.
  - `WorkspaceRenderer<P, T>` — side-effecting, for materializing artifacts, lineage, traces into a workspace.
  - Resolves open question 27.1.
- **`PreferenceRelation` is stateless (§14).** Fitted/stateful models (Bradley-Terry, Plackett-Luce) live on `Population` impls instead — concretely `TournamentPopulation`. Updates happen in `observe_assessment`. Resolves open question 27.6.
- **`ParetoFrontier::partition_filter` builder method (§15.2).** Frontiers can declaratively ignore observations from specific case-set partitions (e.g. only update from `SEARCH`, never from `TEST`). Replaces ad-hoc filter logic in optimizer step bodies.

#### Graph query additions

- **`graph.informed_by(c)` and `graph.informed(c)` (§10.2).** Typed graph relation for "candidates this proposer read from during reflection." Promoted from string-keyed `MetadataBag` access. Avoids the python-gepa stringly-typed metadata-parsing failure mode.

#### Trait law tightening

- **`ContentId` collision resistance (§24).** Strengthened from "observational identity" to "MUST be a cryptographic hash of all observationally-relevant state." Hand-rolled impls are a footgun; ship a derive macro for safe-by-default behavior.

#### Documentation additions

- **`16.5 Workspace lifecycle.`** New full section. `WorkspaceFactory`, `WorkspaceBackend`, `Workspace`, ownership table, standard backends (Local, E2B, Docker, K8s, Firecracker, GitWorktree). Agent runtimes are kept separate from workspaces — they take a workspace and run commands in it.
- **Merge canonicalization (§5.5, §20).** `Artifact::apply` only sees one artifact, so for `Parents::Pair(a, b)` the change canonicalizes to one parent and embeds cross-parent content. Spelled out so readers don't expect the framework to magically combine two artifacts.
- **`ProposalBatchSemantics::Alternatives` cost behavior (§5.7).** All alternatives are evaluated independently if applied successfully. Cost is N×eval, not amortized — the framework does not deduplicate.

#### Plan changes

- **Implementation plan reorders prototypes 2 and 3 (§26).** Pairwise tournament now runs before GEPA parity. Pairwise stresses what is *new* in this design (Pairwise eval requests, fitted preference relations, tournament populations) and is therefore the more informative early test.
- **Two coding-agent worked examples added (§22.4, §22.5).**
  - `gskill`: agentic SWE-smith evaluator with workspace materialization and a reflective proposer.
  - Meta-Harness: agentic proposer reading full graph history via `WorkspaceRenderer`, `Parents::None`, `Arity::None`, multi-axis pareto with `partition_filter`.

#### Stress tests passed

The v0.2 surface was verified against:

1. **Cross-branch synthesis** — proposer reads evidence across two branches, emits a fix as a single proposal with one causal parent and many `informed_by` entries; or two sibling proposals with different parents in one batch.
2. **Meta-Harness end-to-end** — agentic harness search, multi-MB execution traces via `EvidenceStore`, fresh artifacts via `Parents::None`, multi-axis pareto with declarative test-partition filtering.
3. **Workspace lifecycle under k8s and git-worktree backends** — pod-shared, per-workspace containers; worktree-per-workspace with content_id = commit hash; agent commits inside the worktree, framework reads HEAD on cleanup.
4. **Composite multi-agent artifact** — four-agents-and-substrate as one artifact; component-addressed via `Decomposable`; per-component blame attribution via `AttributedEvidence<ComponentId>`. No new primitives required.

20 literature targets from §25 were mentally implemented against this surface. All expressible. The pressure tests surfaced exactly the changes listed above and no others.

### v0.1 second pass (2026-05-05)

Tightened the post-reset design. Major moves: cost as infrastructure not metadata; `ProposalAnnotations` typed vs `MetadataBag` operational; `EvaluationRequest` as a sum type with `Independent / Pairwise / Listwise` variants; `AssessmentGranularity` as an explicit knob; `EvaluatorRegistry` replacing single evaluator; concrete engine shape and run loop; concrete `RunEvent` enum; explicit `CachePolicy` (default `Never`); explicit static-first / dyn-friendly async policy; `EvidenceStore` separating large-evidence persistence from inline graph state.

### v0.1 (2026-05-04, deprecated)

First post-reset draft. Replaced by the second pass.

### v1.0 design lock (2026-05-03, deprecated)

Pre-reset attempt. Six strategy traits, four capability traits, 35+ stdlib impls, multiple coexisting archives, cardinal-only `Score`, capability traits on `Evidence`. Critique surfaced architectural over-engineering; full reset to v0.1.
