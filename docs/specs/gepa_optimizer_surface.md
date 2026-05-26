# Leaven GEPA Optimizer Surface

Status: planning spec.
Date: 2026-05-10.

Supersession note, 2026-05-16: this document is still useful for GEPA layering
intent and early API pressure, but real-GEPA loop semantics, phase boundaries,
defaults, cache parity, and library API choices now live in
`docs/specs/gepa_reference_behavior.md`. Where this document mentions stale
names or defaults such as `SelectedFeedback`, `MinibatchThenValidation` as a
GEPA default, generic population-backed parent selection, or scaffold
reflectors as product reflection, follow the reference behavior document.

This spec defines the product-grade GEPA optimizer Leaven should expose next.
It is the GEPA-specific companion to:

- `docs/specs/gepa_public_private_surface.md`
- `docs/specs/eval_lowering_detail.md`
- `docs/specs/eval_nomenclature.md`

It is subordinate to:

- `docs/specs/initial_library.md`
- `docs/specs/guiding_principles.md`
- root `Cargo.toml` and `crates/leaven/tests/topology_contract.rs` for live
  crate topology
- `docs/specs/milestone_examples_behavioral_contract.md`
- `docs/specs/agentic_stage_runtime.md`
- `docs/specs/agentic_stage_materialization.md`
- `docs/specs/agentic_skill_optimization_primitives.md`
- `docs/specs/agentic_task_execution_substrate.md`
- `docs/specs/agentic_library_user_journey.md`
- `docs/testing/README.md`

When this document conflicts with the public/private surface doc, preserve the
layering rule:

```text
Ordinary users run GEPA with train/validation/test, scorer/evaluator, optimizer,
and budget.

GEPA customizers swap GEPA strategy slots.

Optimizer authors use Optimizer<P>, RunContext, EvaluationRequest, graph views,
and the full substrate.
```

## 1. Problem

Leaven currently has GEPA-shaped primitives and a P3 parity proof, but it does
not yet have an off-the-shelf optimizer library surface.

Today, a user can build a one-step GEPA-like flow by manually writing an
`Optimizer<P>` that evaluates a seed, updates population state, chooses a
candidate, chooses a surface part, proposes an edit, lowers that edit, applies
it, evaluates the child, accepts or rejects it, and updates the population.

That proves the substrate. It is not the library product.

The next GEPA milestone is a reusable `Gepa<P, S, Pop>` optimizer that owns that
rhythm and composes with the public builder described in
`gepa_public_private_surface.md`.

## 2. Product Goal

The ordinary path:

```rust
let result = leaven::optimize(seed_program)
    .train(train_cases)
    .validation(dev_cases)
    .test(test_cases)
    .runner(program_runner)
    .score(score_fn)
    .using(Gepa::default().with_reflection_lm(reflection_lm))
    .budget(Budget::metric_calls(300))
    .run()
    .await?;

let best = result.best();
```

The GEPA customizer path:

```rust
let gepa = Gepa::builder()
    .surface(SkillDirByFrontmatterId)
    .candidate_selector(ParetoFrequencyWeighted::default())
    .part_selector(InvokedAndFailingPart::default())
    .batch_sampler(EpochShuffled::new(4))
    .reflector(LmBackedReflector::with_default_renderer(
        reflection_lm,
        "gpt-4.1-mini",
    ))
    .acceptance(StrictImprovement)
    .validation(FullValidation::every(10))
    .population(ParetoFrontier::by_case())
    .merge(SystemAwareMerge::adaptive())
    .build();
```

The optimizer-author path stays unchanged:

```rust
impl Optimizer<MyProblem> for MyOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, MyProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        // own the optimizer rhythm
    }
}
```

GEPA reflection that runs an agent routes through the optimizer-stage workspace
surface: `AgentBacked<ProposerSlot<_>, Runtime, Bootstrap, Parser>` produces a
typed proposal batch through `RunContext::propose`. `AgentCase` remains the
candidate-evaluation workload surface and must not become the reflection-stage
request type.

GEPA must be a reusable optimizer value. It must not require users to copy the
P3 example's local optimizer loop.

## 3. Upstream Comparator

Python GEPA is useful because its integration burden is small:

```text
candidate: dict[str, str] or str
adapter.evaluate(batch, candidate, capture_traces)
adapter.make_reflective_dataset(candidate, eval_batch, components)
optional adapter.propose_new_texts(...)
```

Leaven should match that product usefulness, not that Python API shape.

Hard cutover rule:

```text
Do not implement Python GEPA adapter compatibility.
Do not flatten Leaven artifacts into dict[str, str] as the primary contract.
Do not introduce a generic string-map candidate layer below typed artifacts.
```

The Rust-native replacement for Python's candidate map is:

```text
P::Artifact                 typed candidate/program state
S: EditSurface<P::Artifact> chosen optimizable projection
S::PartId                   named component/module/field/file/etc
S::View<'a>                 borrowed part view
S::Edit                     surface-native replacement/edit
P::Artifact::Change         artifact-native change after lowering
```

## 4. Candidate Selection And Part Selection

GEPA has two separate selection questions:

```text
candidate selection = which candidate/program version to mutate next
part selection      = where inside that candidate's surface to edit
```

For a prompt artifact with `{ system, rubric, examples }`, candidate selection
chooses the candidate version and part selection chooses one of `system`,
`rubric`, or `examples` inside that version.

The GEPA-facing API name is `candidate_selector`.

## 5. Crate Placement

### 5.1 Ownership

| Crate | Owns | Must not know |
| --- | --- | --- |
| `leaven-core` | artifact/problem/proposal/evaluation/evidence algebra | graph, engine, GEPA, surfaces, LLMs |
| `leaven-surface` | `EditSurface`, `Part`, surface errors/fingerprints | graph, GEPA, stores, workspaces |
| `leaven-engine` | `RunContext`, `RunGraph`, budget, cache, evaluator traits, engine loop, trust/read scopes | GEPA policy, eval product policy, concrete populations, concrete LLM SDKs |
| `leaven-run` | public optimize builder, train/validation/test lowering, scorer/evaluator helpers, default evidence store wiring, result facade | optimizer strategy state, domain semantics, concrete providers/workspaces |
| `leaven-eval` | lowered dataset/split/plan/report data and fingerprints | graph mutation, evaluator execution, GEPA rhythm, environments |
| `leaven-evidence` | scalar/casewise/attribution/pairwise evidence shapes | graph mutation, GEPA rhythm |
| `leaven-population` | `KeepBest`, `TopKFrontier`, `ParetoFrontier`, `TournamentPopulation`, population events | GEPA selectors, graph mutation internals |
| `leaven-engine`/optimizer-owned helpers | renderers/materializers over typed values until a behavior-bearing render crate exists | optimizer rhythm, GEPA policy |
| `leaven-lm` | provider-neutral LM request/response vocabulary | GEPA, engine graph, response-cache stores |
| `leaven-lm-cache` | reusable Leaven response-cache policy, keys, stores, and `CachedLm` wrapper | GEPA rhythm, engine evaluation cache, concrete providers |
| `leaven-gepa` | GEPA optimizer, strategy slots, GEPA request/result types, LM-backed and agent-backed reflection adapters | concrete providers, concrete workspace backends, response-cache stores, domain internals |
| `leaven` | umbrella re-exports only | implementation logic |

### 5.2 Dependency Direction

`leaven-gepa` may depend on:

```text
leaven-kernel
leaven-core
leaven-surface
leaven-engine
leaven-eval
leaven-evidence
leaven-preference
leaven-population
leaven-lm
```

`leaven-gepa` must not depend on:

```text
leaven-lm-openai
leaven-lm-cache
leaven-agent
leaven-agentic
leaven-workspace-*
leaven-artifact-*
future DSRS interop crates
future domain adapter crates
```

`leaven-engine` must not depend on `leaven-gepa`, `leaven-eval`, or
`leaven-run`. Product lowering happens above the engine.

### 5.3 Module Graph In `leaven-gepa`

Planned `leaven-gepa/src/lib.rs` map:

```text
acceptance.rs       Acceptance, AcceptanceDecision, StrictImprovement, ImprovementOrEqual, NoRegression
batch.rs            BatchSampler, EpochShuffled, FixedMinibatch
error.rs            GepaError, GepaBuilderError, GepaPolicyError
gepa.rs             Gepa, Optimizer<P> impl, private step state
merge.rs            MergeScheduler, SystemAwareMerge, GepaMerge
candidate_selector.rs  CandidateSelector, ParetoFrequencyWeighted, SelectBestCandidate, UniformFrontier, TopK
part_selector.rs    PartSelector, RoundRobinPart, InvokedAndFailingPart
proposal.rs         GepaMutationRequest, GepaProposal, SurfaceEdit
reflection.rs       ReflectRequest, SelectedFeedback, LmBackedReflector, reflection prompt construction, ASI rendering
result.rs           GepaSummary, candidate summaries, frontier summaries
split_policy.rs     GepaSplitPolicy, train/validation/test defaults over PartitionId
validation.rs       ValidationPolicy, FullValidation, MinibatchThenValidation
```

`lib.rs` remains a map only.

## 6. Core API Shape

Canonical shape:

```rust
pub struct Gepa<
    P,
    S,
    Pop = ParetoFrontier,
    CandidateSel = ParetoFrequencyWeighted,
    PartSel = RoundRobinPart,
    Batch = EpochShuffled,
    Reflect = LmBackedReflector,
    Accept = StrictImprovement,
    Validate = MinibatchThenValidation,
> where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    surface: S,
    population: Pop,
    candidate_selector: CandidateSel,
    part_selector: PartSel,
    batch_sampler: Batch,
    reflector: Reflect,
    acceptance: Accept,
    validation: Validate,
    merge: Option<MergeScheduler<P, S>>,
    split_policy: GepaSplitPolicy,
    config: GepaConfig,
    state: GepaState,
}
```

The concrete code may keep slots generic rather than object-safe when that
preserves stronger types. The public builder should hide generic noise for
ordinary use.

`S` stays static. Its `PartId`, `Edit`, and fingerprint are part of proposal
typing, attribution typing, cache safety, and surface-edit lowering.

## 7. Builder Requirements

`Gepa::builder()` must support:

```text
surface(S)
population(Pop)
candidate_selector(...)
part_selector(...)
batch_sampler(...)
reflector(...)
reflection_lm(...)
acceptance(...)
validation(...)
merge(...)
max_metric_calls(...)
max_iterations(...)
seed(u64)
proposal_count(usize)
with_profile(GepaProfile::Reference | GepaProfile::OptimizeAnything | GepaProfile::FastCertified)
track_best_outputs(bool)
track_candidate_history(bool)
split_policy(...)
```

The builder must reject incomplete or contradictory configurations before the
run starts:

- no surface and no derivable default surface;
- no reflector/proposer and no reflection LM default path;
- validation policy references a missing partition;
- train/search partition is empty when the sampler needs cases;
- validation/test partitions overlap train unless split policy permits overlap;
- batch sampler cannot draw from the configured search set;
- merge enabled without enough candidate lineage/support requirements.

Default surface derivation is explicit:

```rust
pub trait DefaultEditSurface<A: Artifact> {
    type Surface: EditSurface<A>;
    fn default_surface() -> Self::Surface;
}
```

`Gepa::default()` may be used in public examples only when the artifact/domain
adapter supplies this contract or the product builder supplies a surface. There
is no implicit string split, prompt-section split, or whole-artifact mutation
fallback unless a concrete surface type implements it.

## 8. GEPA Step Contract

Named profiles are presets over this step contract, not separate hidden
engines. `GepaProfile::Reference` keeps the upstream-compatible certified loop:
epoch minibatch 3, one serial proposal, skip-perfect enabled, and full
validation before admission. `GepaProfile::OptimizeAnything` keeps the same
certified loop but disables skip-perfect to match upstream optimize-anything
defaults such as the AIME example. `GepaProfile::FastCertified` is the first
opt-in speed profile: it uses smaller train probes and multiple serial proposal
attempts per selected parent while preserving full validation before reference
admission. It is deliberately not the future lazy-certification / async-island
FastGEPA design; those variants must land as explicit new policies with report
fields that distinguish uncertified, approximately certified, and fully
certified candidates.

One ordinary reflective mutation iteration is:

```text
1. Ensure seeds are inserted and population has observed seed baseline.
2. Build a population view from the current graph.
3. CandidateSelector chooses candidate(s).
4. PartSelector chooses one or more surface parts on the parent artifact.
5. BatchSampler chooses a feedback minibatch from the train/search split.
6. GEPA evaluates the parent on the minibatch with required granularity.
7. GEPA extracts/captures feedback assessment IDs.
8. The configured reflector/proposer proposes one or more edits/native proposals.
9. GEPA lowers surface edits through EditSurface::change_part.
10. GEPA records a ProposalBatch with typed causal and informed_by provenance.
11. GEPA applies the batch through RunContext.
12. GEPA evaluates children on the same minibatch.
13. Acceptance decides which children deserve validation/admission.
14. ValidationPolicy chooses validation/search request for admitted children.
15. GEPA evaluates admitted children as required.
16. Population observes candidate/assessment IDs explicitly.
17. GEPA emits iteration events and either continues or returns Done.
```

Required invariants:

- graph mutations go only through `RunContext`;
- per-case assessment rows are normalized into casewise evidence before any
  `ParetoFrontier` update;
- validation/test case content is hidden from reflective proposers by default
  through `leaven-run` lowering into engine trust/read policy;
- proposer feedback uses `InfoRef::Assessment`, `InfoRef::Candidate`, or
  external refs, not stringly metadata keys;
- every accepted child has causal lineage through `CausalInputs`;
- every reflection/proposal records what it read through `informed_by`;
- acceptance rejection does not erase graph truth about the proposal, apply
  attempt, or screening assessment;
- population events are optimizer opinions, not graph truth;
- GEPA treats `EvaluationCompleted.assessment_ids` as the full per-case row set
  and never treats `assessment_ids[0]` as a bundled minibatch assessment.

## 9. Evaluation And Data Semantics

GEPA consumes the lowered eval layer from `docs/specs/eval_lowering_detail.md`.
It does not make users construct that layer directly.

Default GEPA split behavior:

| Split role | Reflection feedback | Candidate selection | Part selection | Acceptance/admission | Population | Final report |
| --- | --- | --- | --- | --- | --- | --- |
| Train/Search | yes | yes | yes | yes | yes | yes |
| Validation | no by default | optional explicit policy | optional explicit policy | optional explicit policy | no by default | yes |
| Test | no | no | no | no | no | post-loop only |
| Probe | explicit only | explicit only | explicit only | explicit only | explicit only | optional |

The public explanation is:

```text
training data drives the search;
validation is held out unless you explicitly select a validation-aware policy;
test is final-report-only by default.
```

The lowered implementation may use `Dataset`, `DatasetSplits`,
`EvaluationPlan`, `EvaluationRequestTemplate`, `TrustPolicy`, and
`EvaluationRequest`. Those terms belong in builder/optimizer-author docs, not
the ordinary GEPA example.

## 10. Surface And Proposal Contract

### 10.1 Surface Edit

```rust
pub struct SurfaceEdit<S, A>
where
    A: Artifact,
    S: EditSurface<A>,
{
    pub part: S::PartId,
    pub edit: S::Edit,
}
```

Surface edits are GEPA's default proposal payload. Artifact-native proposals
remain allowed for specialized proposers.

### 10.2 GEPA Proposal

```rust
pub enum GepaProposal<P, S>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    SurfaceEdit {
        target: CandidateId,
        edit: SurfaceEdit<S, P::Artifact>,
        annotations: P::ProposalAnnotations,
        informed_by: Vec<InfoRef>,
    },
    Native(Proposal<P>),
}
```

GEPA lowers `SurfaceEdit` into `Proposal::mutate(...)` or
`Proposal::merge(...)` by calling:

```rust
surface.change_part(parent_artifact, part, edit)
```

### 10.3 Merge Canonicalization

GEPA merge reads two parent artifacts through the same surface. It picks one
parent as the apply target and lowers imported content into that target's
native `Change`.

Graph lineage records:

```text
effect: Change { target: left, change }
causal: Pair(left, right)
informed_by: [Candidate(left), Candidate(right), ...]
```

There is no magical two-artifact `apply_change`.

## 11. Reflection And ASI

Python GEPA's `make_reflective_dataset` becomes a renderer/proposer concern,
not a universal adapter contract.

GEPA has two distinct LM interaction sites:

```text
candidate execution LM
  owned by the runner/evaluator/domain adapter
  runs a candidate artifact on cases and produces outputs, scores, traces, evidence

reflection LM
  owned by the GEPA reflection proposer
  reads selected feedback and proposes a typed candidate change
```

These may be the same provider instance in an application, but Leaven must not
couple them in the type system. A DSRS or AIME runner that calls an LM is still
candidate evaluation. The GEPA reflection LM is the proposer backend that turns
feedback into candidate edits.

### 11.1 Shared Reflection Request

LM-backed and agent-backed GEPA reflection use the same GEPA-owned request
vocabulary. The shared request types live in `leaven-gepa`, not
`leaven-stage`, because they are GEPA strategy input, not workspace substrate.

```rust
pub struct ReflectRequest<Part = String> {
    pub parent: CandidateId,
    pub part: Part,
    pub part_label: String,
    pub selected_feedback: SelectedFeedback,
}

pub struct SelectedFeedback {
    pub assessment_refs: Vec<AssessmentId>,
    pub evidence_refs: Vec<InfoRef>,
    pub candidate_refs: Vec<CandidateId>,
    pub records: Vec<ReflectiveFeedbackRecord>,
}

pub struct ReflectiveFeedbackRecord {
    pub case: Option<CaseId>,
    pub score: Option<f64>,
    pub output: Option<String>,
    pub feedback: String,
    pub source_refs: Vec<InfoRef>,
}
```

`source_refs()` over `SelectedFeedback` is the authoritative provenance input
for `Proposal::informed_by`. The textual records are proposer input. The refs
are graph/evidence truth.

The shared request is intentionally not `AgentCase`, not a workspace plan, and
not a Python-style `dict[str, str]`. It is selected GEPA feedback for a parent
candidate and selected surface part.

The default `String` part type keeps agent-stage JSON requests small. Typed
LM-backed reflectors use their surface's native `S::PartId` so parsing and
lowering cannot drift from `EditSurface::change_part`.

### 11.2 Feedback Selection

GEPA owns the conversion from scored per-case assessment rows into selected
reflection records. `CasewiseEvidence` is the normalized GEPA view over those
rows, not the required evaluator output shape for `AssessmentGranularity::PerCase`.

```rust
pub trait GepaCaseEvidence: leaven_core::Evidence {
    fn scalar_score(&self) -> Option<ScalarEvidence>;
    fn reflection_record(&self) -> Option<ReflectiveFeedbackRecord>;
}
```

The standard implementation for `CaseAssessmentEvidence` preserves:

```text
case id
comparable scalar score
CaseAssessmentEvidence::output()
CaseAssessmentEvidence::feedback()
assessment/evidence source refs
```

Do not collapse feedback to `f64` before reflection. Scalar scores drive
population and acceptance; generated output and feedback text drive reflection.

Every selected record carries the row's `InfoRef::Assessment`. The reflection
request also carries `InfoRef::Candidate(parent)` and the complete selected row
set so proposal lineage can be audited without relying on a synthetic aggregate
assessment.

The default feedback selector uses only train/search assessments under the
current trust policy. Validation and test evidence are excluded from
`SelectedFeedback` unless the user selects an explicit validation-aware policy.

### 11.3 LM-Backed Reflector

Standard reflective mutation is an LM-backed proposer over the neutral
`leaven-lm::Lm` capability:

```rust
pub struct LmBackedReflector<L, R, Parser> {
    lm: L,
    model: ModelName,
    renderer: R,
    parser: Parser,
    config: LmBackedReflectorConfig,
}
```

`leaven-gepa` constructs no concrete provider. The actual LM is injected by the
caller as an `impl leaven_lm::Lm`:

```rust
let reflector = LmBackedReflector::new(
    OpenAiLm::from_env()?,
    ModelName::new("gpt-4.1-mini"),
    renderer,
    parser,
);
```

Tests and examples use deterministic `Lm` fixtures. Applications that want
response caching wrap the provider before injection:

```rust
let lm = CachedLm::read_write(OpenAiLm::from_env()?, cache);
let reflector = LmBackedReflector::new(lm, "gpt-4.1-mini", renderer, parser);
```

`leaven-gepa` still depends only on `leaven-lm`; it does not depend on
`leaven-lm-openai` or `leaven-lm-cache`.

The call is exactly:

```rust
let request = LmRequest::new(
    self.model.clone(),
    Messages::from_user(rendered_reflection_prompt),
)
.with_sampling(self.config.sampling.clone())
.with_output(self.config.output.clone());

let metered = self.lm.complete(request).await?;
let assistant_text = metered.value.assistant.content();
let cost = metered.cost;
```

The renderer produces the `system_prompt` and `user_prompt` from:

```text
candidate id
selected surface part
surface part view
screening/minibatch assessment row IDs
GEPA-normalized casewise evidence
optional attribution evidence for the selected part
lineage summary
objective/background prompt text
```

The parser consumes only the assistant text and the selected surface context. It
returns either surface-native edits or a typed `ProposalBatch<P>`. Invalid LM
output is a proposal error, not a panic and not a hidden no-op.

The default renderer/parser pair follows upstream GEPA's ordinary instruction
proposal surface: the renderer places the selected part text in `<curr_param>`,
renders selected feedback into `<side_info>`, and asks for replacement
instructions inside triple-backtick fences. `PlainTextEditParser` extracts the
first fenced replacement when present and otherwise falls back to stripped raw
assistant text. JSON is a custom renderer/parser choice, not the default
reflection contract.

### 11.4 Proposer Finalization Path

LM-backed reflection should use the same engine proposer finalization path as
agent-backed reflection:

```text
GEPA builds ReflectRequest
  -> RunContext::propose(&lm_backed_proposer_adapter, request)
  -> LmBackedReflector renders LmRequest
  -> impl Lm::complete returns Metered<LmResponse>
  -> parser returns ProposalBatch<P>
  -> RunContext records ProposalBatch with LM cost
  -> GEPA calls RunContext::apply_batch
```

The LM-backed proposer adapter implements
`Proposer<P, Request = ReflectRequest<S::PartId>>` and returns a
`Metered<ProposalBatch<P>>`. It is GEPA-owned because it needs the selected
`EditSurface` to lower parsed output into artifact changes. `RunContext::propose`
is responsible for proposal recording, event emission, budget charging, and
checkpoint interaction. The GEPA loop remains responsible for `apply_batch`,
child screening, acceptance, validation, and population updates.

`GepaReflector` may remain the optimizer-facing convenience trait, but when the
concrete reflector is backed by a proposer adapter it should call
`RunContext::propose` rather than manually calling `record_proposal_batch`.

### 11.5 Agent-Backed Reflector

Agent-backed reflection is optional. It is selected when the user supplies an
`AgentBacked<ProposerSlot<ReflectRequest>, Runtime, Bootstrap, Parser>` or a
GEPA-owned wrapper around that type.

Agent-backed reflection consumes the same `ReflectRequest`, but `AgentBacked`
turns it into:

```text
AgentStagePlan<ReflectRequest>
bounded workspace setup
prewarmed StageQueryPolicy entries
AgentRuntime session
StageOutputParser output
mandatory StageAttemptReceipt
Metered<ProposalBatch<P>>
```

This path is governed by `agentic_stage_materialization.md`. It must not be the
only production reflection path. LM-backed reflection is the default no-workspace
GEPA reflection path.

### 11.6 ASI / Feedback Sources

ASI/feedback sources:

- evaluator evidence fields;
- casewise scalar outcomes;
- attribution evidence keyed by `S::PartId`;
- command/stdout/stderr evidence when the evaluator records it;
- transcript refs from agentic evaluators;
- validation/apply errors;
- previous successful candidate summaries.

Do not add a global `oa.log` equivalent in `leaven-gepa`. Logging capture is
stage/evaluator evidence policy. A closure-based helper can provide ergonomic
stdout/log capture later, but that belongs over `Evaluator<P>`, not in GEPA
core behavior.

## 12. Result Contract

`GepaSummary` is an ID/ref-only summary over graph truth plus optimizer state,
not a second source of truth.

Minimum shape:

```rust
pub struct GepaSummary {
    pub best: Option<CandidateId>,
    pub seed: CandidateId,
    pub population_id: PopulationId,
    pub iterations: u64,
    pub total_cost: Cost,
    pub candidates: Vec<GepaCandidateSummary>,
    pub parents: Vec<GepaLineageSummary>,
    pub frontier: GepaFrontierSummary,
    pub rejected: Vec<GepaRejectionSummary>,
    pub evaluation_report: Option<EvaluationReport>,
}
```

Convenience methods may return artifacts through a graph view, but the result
must not copy artifacts or evidence payloads.

## 13. Stop, Budget, Cache, And Checkpoint

Required standard controls:

```text
max_iterations
max_metric_calls
max_lm_calls
max_cost
max_wall_time
external stop callbacks through engine callback/stopper surface
```

Budget rules:

- evaluator calls charge through `RunContext::evaluate`;
- reflection LM calls charge as proposer/reflection cost;
- merge evaluation charges through the same evaluation path;
- proposal generation must fail before graph mutation if budget is exhausted.

Caching rules:

- evaluator cache remains engine-owned;
- cache keys use evaluator fingerprint, resolved evaluation set, request shape,
  and artifact cache identities;
- surface fingerprint participates only when rendering/evaluation actually
  depends on the surface projection.

Checkpoint rules:

- graph truth is enough to reconstruct public history;
- GEPA private state must implement `CheckpointableOptimizer` before
  long-running/resumable runs are considered product-ready;
- checkpointed private state includes RNG, batch sampler cursor, parent/part
  selector state, gate/admission state, merge scheduler state, and population
  state if those values are not fully derivable from graph events.

## 14. Concrete Requirements

The first product-grade GEPA implementation must:

1. implement `Optimizer<P>` for `Gepa<P, S, Pop>`;
2. support seed insertion and seed baseline evaluation;
3. support single-task, train-only, and train+validation runs;
4. support explicit test splits as final-report-only by default;
5. select parents through a swappable `CandidateSelector`;
6. select parts through a swappable `PartSelector`;
7. sample minibatches through a swappable `BatchSampler`;
8. propose surface edits through `LmBackedReflector` or another supplied reflector;
9. lower surface edits into typed artifact changes;
10. record proposal batches with typed causal and informational provenance;
11. apply proposals through `RunContext`;
12. screen children on minibatches;
13. accept or reject children through a swappable `Acceptance`;
14. validate/admit through a swappable `ValidationPolicy`;
15. update population explicitly;
16. return best candidate through population state;
17. expose GEPA summaries and lowered evaluation report slots;
18. respect budget and trust scopes.

## 15. Defaults

Defaults:

```text
population:        ParetoFrontier::by_case()
candidate selector:   ParetoFrequencyWeighted
part selector:     RoundRobinPart
batch sampler:     EpochShuffled { minibatch_size: 3 }
acceptance:        StrictImprovement
validation:        MinibatchThenValidation over search/train unless configured
split policy:      train/search feedback, validation held out, test final-only
merge:             disabled
proposal count:    1
```

Single-task defaults may use `KeepBest` instead of `ParetoFrontier` when no
case axis exists.

## 16. Errors

GEPA must return typed errors for:

- no seed and no seedless generator configured;
- no parent selected from population;
- selected parent missing from graph;
- surface has no parts;
- part disappeared between selection and lowering;
- proposer output references unknown part;
- proposer output is invalid for the surface;
- evaluator does not support requested granularity;
- expected case-targeted assessment rows missing;
- a per-case GEPA evaluation returns an aggregate/set-targeted assessment;
- acceptance policy cannot compare requested evidence shape;
- split data references unknown cases;
- required train/search split is empty;
- disjoint split policy is violated;
- validation policy requests forbidden split;
- test split is requested for feedback/admission under default policy;
- trust policy denies proposer read;
- budget exhausted before proposal/evaluation mutation.

Do not use `OptimizerError::Message` for known public failures once the error
shape is known.

## 17. Tests And Acceptance

Each test must name a claim and live at the lowest clean layer.

### 17.1 `leaven-gepa` Law/Example Tests

- builder rejects missing surface/proposer/evaluator-required config;
- `RoundRobinPart` cycles deterministically over a stable surface;
- `ParetoFrequencyWeighted` samples only population/frontier members;
- surface edit lowering changes only the selected part for a generated
  `PartMapArtifact`;
- acceptance policies implement strict/equal/no-regression laws;
- batch sampler is deterministic under a seed;
- reflective proposer turns casewise feedback into a surface edit using a
  deterministic `Lm` fixture;
- invalid proposer output becomes typed proposal error, not panic;
- merge canonicalizes to one target while preserving pair causal lineage;
- split policy refuses test split as feedback/admission by default;
- private checkpoint state captures/restores RNG and sampler cursor.

### 17.2 Product Scenario Tests

Under `crates/leaven-run/tests` or `crates/leaven/tests`:

- single-task GEPA optimizes a one-string artifact with `KeepBest`;
- multi-task GEPA improves a two-part artifact through `ParetoFrontier`;
- generalization GEPA hides validation from proposer and still reports
  validation evaluation;
- test cases run only in post-loop final reporting under default policy;
- dataset split fingerprint changes when case membership changes;
- rejected candidates remain visible in graph but absent from population;
- result best candidate matches population best;
- callback/event order includes proposal, apply, evaluation, population, and
  optimization end events.

### 17.3 Example Packages

Existing:

```text
examples/p3_gepa_parity
```

New product examples:

```text
examples/p8_aime_gepa
examples/p9_gepa_skill_surface_smoke
examples/p10_eval_suite_train_val_test
```

`p8` is the minimal off-the-shelf AIME prompt optimizer: deterministic scripted
runner by default, optional HuggingFace AIME cache, and opt-in live OpenAI
runner. `p9` should prove GEPA over a folder/skill-like surface with mock
LM/runtime only.

## 18. Implementation Milestones

### Milestone 0: Eval Lowering Substrate

Goal: make train/validation/test semantics reusable before GEPA product
ergonomics depend on them.

Scope:

- scaffold `leaven-eval` with `Dataset`, `DatasetSplits`,
  `EvaluationPlan`, `SplitUsePolicy`, and `EvaluationReport`;
- map product-builder `.train`, `.validation`, and `.test` into stable
  partitions in `leaven-run`;
- prove evaluation can run without a dataset and without an environment;
- add one adapter path from `leaven-agentic` into split/report summaries
  without moving agentic internals into `leaven-eval`.

### Milestone A: Real GEPA Loop, Deterministic Proposer

Goal: replace example-local GEPA rhythm with reusable `Gepa<S, Pop, ...>` that
does not require Layer 1 callers to name the lowered problem type.

Scope:

- `Gepa` implements `Optimizer<P>`;
- deterministic proposer returns configured surface edit;
- builder supports surface/population/parent/part/batch/acceptance/validation;
- P3 example becomes thin setup code using `Gepa` directly.

### Milestone B: Mock-LM-Backed Reflector

Goal: prove reflection loop without provider network calls.

Scope:

- `LmBackedReflector` uses `leaven-lm` trait vocabulary;
- a deterministic `Lm` fixture drives proposal text;
- standard reflection renderer consumes casewise evidence and part view;
- typed parse/validation errors become proposer feedback.

### Milestone C: Product Entry Builder

Goal: make the short user path work without making `leaven-engine` depend on
`leaven-eval`.

Scope:

- scaffold `leaven-run`;
- public builder accepts seed/train/validation/test/runner/scorer/evaluator/optimizer;
- builder lowers to engine `CaseSet`, evaluators, trust policy, budget,
  callbacks, evidence store, and optimizer;
- `leaven` re-exports `leaven_run::optimize` as the product entrypoint while
  engine builder remains available through `leaven::engine`.

### Milestone D: Merge, Cache, Checkpoint

Goal: parity with upstream's useful long-run features.

Scope:

- `SystemAwareMerge`;
- merge scheduler state;
- cache-aware full/minibatch evaluation path;
- `CheckpointableOptimizer` for GEPA private state.

### Milestone E: Agentic/Skill Surface Proof

Goal: prove Leaven's typed surface beats Python's string-map adapter.

Scope:

- skill/folder artifact surface;
- mock agentic evaluator;
- trace attribution feeds `InvokedAndFailingPart`;
- validation split hidden from proposer;
- test split final-report-only by default;
- shared evaluation report summarizes train/validation/test outcomes.

## 19. Verification

During implementation:

```bash
cargo nextest run -p leaven-gepa
cargo nextest run -p leaven-eval
cargo nextest run -p leaven-run
cargo nextest run -p leaven --test gepa_parity
cargo run -p p3_gepa_parity
```

Completion:

```bash
just check
```

## 20. Non-Goals

- Python GEPA API compatibility.
- A second engine loop inside `leaven-gepa`.
- `dict[str, str]` as the core candidate representation.
- GEPA-specific fields on `Artifact`.
- GEPA-specific hooks inside `leaven-engine`.
- Concrete OpenAI/Anthropic dependencies in `leaven-gepa`.
- Hidden validation/test use for convenience.
- Moving rich agentic case/workspace semantics into `leaven-eval`.
- Public test holes.
- Compatibility aliases for old names.

## 21. Open Decisions

1. Whether closure evaluator helpers live in `leaven-run` only or also get
   optimizer-specific helpers in `leaven-gepa`.
2. Whether single-task GEPA defaults to `KeepBest` or a degenerate
   single-axis `ParetoFrontier`.
3. Whether validation scores may influence default selection. The conservative
   default is report-only unless the user selects a validation-aware policy.
