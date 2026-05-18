# GEPA AIME Paper-Parity Execution

Status: implementation spec.

This spec defines the Leaven behavior required to run a GEPA AIME
paper-parity experiment through the public optimizer library surface. It builds
on:

- `docs/specs/gepa_public_private_surface.md`
- `docs/specs/gepa_optimizer_surface.md`
- `docs/specs/gepa_reference_behavior.md`
- `docs/specs/durable_runs_and_resume.md`
- `docs/specs/lm_runtime_and_response_cache.md`
- `docs/specs/eval_lowering_detail.md`
- `docs/specs/case_visibility_and_target_isolation.md`
- `docs/specs/aime_case_report_adapter.md`
- `docs/specs/gepa_reflection_evidence_visibility.md`
- `docs/specs/p8_run_report_operator_ux.md`
- `docs/specs/p8_live_provider_budget_reliability.md`

The immediate paper target is the GEPA CAIS AIME Math artifact under
`~/vendor/github.com/gepa-ai/gepa-cais26-artifact/acm_cais_artifact_evaluation/domains/aime_math`.
The adjacent reference implementation is
`~/vendor/github.com/gepa-ai/gepa/examples/aime_math`.

Algorithm authority: `gepa_reference_behavior.md` owns the definition of real
GEPA. This AIME spec owns the AIME dataset, runner, scorer, model-role, report,
and operator-path requirements. If this document describes train-only parent
selection, optional accepted-candidate validation, request-level-only evaluation
caching, or a different GEPA default, treat that text as superseded by
`gepa_reference_behavior.md`.

## 1. Product Outcome

A user can run GEPA over AIME through the ordinary Leaven surface:

```rust
let result = leaven::optimize(AimePrompt::new(GEPA_AIME_SEED_PROMPT))
    .train(train)
    .validation(validation)
    .test(test)
    .runner(aime_solver)
    .score(aime_score)
    .using(Gepa::for_surface(AimePromptSurface))
    .budget(Budget::metric_calls(500))
    .run()
    .await?;
```

The run must be durable by default, resumable from clean boundaries, stopped by
metric-call budget, and backed by Leaven LM/provider/cache infrastructure.

This spec requires the default live P8 reflection model to use Leaven's
`gpt-5.4-mini` medium-reasoning control. This intentionally differs from the
GEPA optimize-anything AIME example's `gpt-5.1` reflection setting; material
deltas from the GEPA artifact must be documented in the P8 report and example
README.

Operators may set `LEAVEN_AIME_REFLECTION_MODEL=gpt-5.1` for a strict
upstream-reflector comparison run. That changes only the live reflection role
model; it must remain on the same public `optimize(seed).using(Gepa...)` path,
and the report must classify the actual reflection model as either
`upstream-matched` or `model-delta`.

## 1.1 User-Facing API Contract

The ordinary P8 live path must be expressible without engine internals:

```rust
let cases = AimeDataset::load(cache_path)?;

let result = leaven::optimize(AimePrompt::seed())
    .train(cases.train)
    .validation(cases.validation)
    .test(cases.test)
    .runner(AimeSolver::openai(OpenAiLm::from_env("gpt-4.1-mini")?))
    .score(AimeScorer::exact_integer())
    .using(
        Gepa::builder()
            .surface(AimePromptSurface)
            .reflection_lm(OpenAiLm::from_env("gpt-5.4-mini")?)
            .aime_defaults()
            .build()?,
    )
    .budget(Budget::metric_calls(500))
    .run()
    .await?;
```

Public users must not need to mention:

- `RunGraph`;
- `RunContext`;
- `EvaluationRequest`;
- `ResolvedEvaluationSet`;
- `ReadScope`;
- `CheckpointableOptimizer`;
- GEPA checkpoint state;
- engine `Stopper` trait objects.

Power users may replace GEPA slots through advanced GEPA builder methods, but
the off-the-shelf AIME path must have defaults.

## 1.2 Implementation Slices

This work has five ordered slices. Later slices must not fake earlier slices:

1. **Durable run substrate already completed or assumed available**: ordinary
   run checkpoints include optimizer continuation and resume refuses missing or
   incompatible continuation state.
2. **Engine stop semantics**: budget-derived stop reason and stopper loop.
3. **GEPA loop semantics**: sampler, parent selection, acceptance, validation
   policy, cache, best-output trace, continuation.
4. **Reflection and LM semantics**: renderer, parser, LM/cache/provider path.
5. **AIME operator path**: dataset materialization, runner, scorer, P8 command,
   live smoke/full run.

No slice may claim paper-parity readiness by itself.

## 2. Reference Hyperparameters

GEPA AIME reference settings:

| Setting | Required Leaven value |
| --- | --- |
| Seed prompt | `Solve the math problem carefully. Break down the steps and provide the final answer as a single number.` |
| Train/validation dataset | `AI-MO/aimo-validation-aime` split into train and validation |
| Held-out test dataset | `MathArena/aime_2025` |
| Solver model | `gpt-4.1-mini` |
| Solver temperature | `1.0` |
| Solver max output tokens | `32000` |
| Search budget | `max_metric_calls=500` equivalent |
| Parallel execution | enabled |
| Max workers | `32` |
| Evaluation cache | enabled |
| Track best outputs | enabled or explained by richer durable trace |
| Reflection prompt parser | fenced text replacement, not JSON |
| Reflection model | Leaven default `gpt-5.4-mini` with medium reasoning, overridable |

The GEPA CAIS artifact reports AIME Math test accuracy from `46.67%` to
`60.00%`, with validation reaching `57.78%`. Leaven must treat those numbers as
the reproduction target, not as a result until a live run proves them.

## 3. Budget And Stop Semantics

### 3.1 Search Budget

`Budget::metric_calls(500)` in the AIME GEPA path maps to the optimization
search budget. It must not be consumed by final validation/test report work.

If final report work needs its own limits, the builder must expose a separate
report/evaluation budget or use an explicit unlimited report budget. It must not
silently steal search budget from GEPA.

### 3.2 Clean Budget Stop

The engine must check a budget-derived stopper before each optimizer step:

```text
if spent.metric_calls >= search_budget.metric_calls {
    stop with StopReason::BudgetReached
}
```

Required behavior:

- no new optimizer step starts after the stopper trips;
- no new proposal or train evaluation is recorded after the stopper trips;
- the run returns the current best candidate when one exists;
- the result records stop reason and budget snapshot;
- durable state is checkpointed at the clean stop boundary;
- resume from this stored run does not repeat committed evaluations.

`BudgetExceeded` remains a hard in-stage guard. It is not the normal GEPA
`max_metric_calls` stop path.

### 3.2.1 Required Types

The final names may differ, but the implementation must preserve these
distinctions:

```rust
pub enum StopReason {
    OptimizerDone,
    BudgetReached(BudgetReached),
    ExternalStop(ExternalStopReason),
    Error,
}

pub struct BudgetReached {
    pub dimension: BudgetDimension,
    pub cap: u64,
    pub spent: u64,
    pub stage: Option<StageId>,
}

pub struct StopContext<'a, P: OptimizationProblem> {
    pub graph: RunGraphView<'a, P>,
    pub budget: BudgetSnapshot,
    pub iteration: Option<IterationId>,
    pub optimizer_summary: Option<OptimizerSummary>,
}

pub enum StopDecision {
    Continue,
    Stop(StopReason),
}
```

`BudgetReached` is a clean stop condition. `BudgetExceeded` is a refusal to
charge attempted work. They must remain separate types and event paths.

The default high-level GEPA path may lower `Budget::metric_calls(500)` to a
search stopper internally, but user docs should continue to teach `.budget(...)`
rather than a separate stopper object.

### 3.2.2 Search Budget Vs Report Budget

`leaven-run` must distinguish at least:

```rust
pub struct RunBudgetPolicy {
    pub search: Budget,
    pub final_report: FinalReportBudget,
}

pub enum FinalReportBudget {
    Unlimited,
    SameLedger,
    Separate(Budget),
}
```

For GEPA AIME parity, `Budget::metric_calls(500)` means
`RunBudgetPolicy { search: metric_calls(500), final_report: Unlimited }` unless
the caller explicitly configures otherwise.

Final validation/test report work must be tagged separately in the budget
report so users can see search spend and report spend.

### 3.3 Parallel Overshoot

When parallel work is already scheduled, observed metric calls may exceed the
cap by the number of in-flight jobs. This is acceptable only if:

- the overshoot is caused by already-scheduled jobs;
- no new optimizer step is scheduled after the cap is observed;
- the report records spent calls honestly.

## 4. GEPA Loop Requirements

GEPA must run as a real iterative optimizer. The one-iteration scaffold is not
an ordinary GEPA default.

### 4.0 Required Loop Order

Each GEPA optimizer step must follow this order:

1. Check engine stoppers before entering the step.
2. Select parent candidate from population/frontier, falling back to seed only
   when no admissible population candidate exists.
3. Select train batch from sampler.
4. Resolve selected surface part(s).
5. Evaluate or load cached parent evidence on selected train batch.
6. Render reflection materials from parent artifact, selected part, selected
   case feedback, traces, and objective context.
7. Call reflector/proposer through Leaven LM or configured non-LM proposer.
8. Parse proposal output into a surface edit or typed proposal.
9. Record proposal batch through `RunContext`.
10. Apply proposal through `RunContext`.
11. Evaluate or load cached child evidence on selected train batch.
12. Run acceptance on parent/child evidence summaries.
13. If accepted, update population/frontier and best state.
14. Run validation policy if cadence says to evaluate accepted candidates.
15. Persist clean checkpoint with graph, budget, cache, and GEPA continuation.

Steps 5 and 11 may be cached. Cache hits must not charge new metric calls.

No stage may mutate graph directly. Graph mutation stays behind `RunContext`.

### 4.0.1 GEPA Continuation State

GEPA continuation must contain all state needed to make the same next decision
after restore:

```rust
pub struct GepaContinuation<Pop, ParentSel, PartSel, Batch, Accept, Valid, Merge, Stop> {
    pub search_partition: PartitionId,
    pub completed_steps: u64,
    pub proposal_count: u64,
    pub best: Option<CandidateId>,
    pub observed: BTreeSet<CandidateId>,
    pub population: Pop,
    pub parent_selector: ParentSel,
    pub part_selector: PartSel,
    pub batch_sampler: Batch,
    pub acceptance: Accept,
    pub validation_policy: Valid,
    pub merge_scheduler: Option<Merge>,
    pub stopper_state: Option<Stop>,
}
```

GEPA must fail resume before running if any candidate or assessment referenced
by continuation is missing from graph truth.

### 4.1 Train Batch Sampling

GEPA must select train cases through a sampler.

Required trait shape:

```rust
pub trait BatchSampler<P: OptimizationProblem>: Send + Sync {
    type State: Serialize + DeserializeOwned;
    type Error: std::error::Error + Send + Sync + 'static;

    fn sample(
        &mut self,
        ctx: BatchSampleContext<'_, P>,
    ) -> Result<SampledBatch, Self::Error>;

    fn checkpoint_state(&self) -> Self::State;
    fn restore_state(&mut self, state: Self::State) -> Result<(), Self::Error>;
}

pub struct BatchSampleContext<'a, P: OptimizationProblem> {
    pub graph: RunGraphView<'a, P>,
    pub split: &'a DatasetSplits,
    pub partition: &'a PartitionId,
    pub budget: &'a BudgetSnapshot,
    pub rng: &'a mut dyn RngCore,
}

pub struct SampledBatch {
    pub partition: PartitionId,
    pub case_ids: Vec<CaseId>,
    pub purpose: EvaluationPurpose,
}
```

Default AIME sampler:

- samples from `TRAIN`;
- batch size matches the GEPA default or is documented if Leaven chooses a
  fixed first implementation;
- never samples validation/test;
- records sampled case ids in the proposal/reflection materials.

Required sampler behavior:

- samples from train/search split only by default;
- records selected case ids in graph evidence/proposal context;
- is deterministic under stored run seed/continuation state;
- snapshots any RNG, cursor, or adaptive sampler state in GEPA continuation.

### 4.2 Parent Selection

Parent selection must operate over GEPA population/frontier state and a scoped
graph view.

Required trait shape:

```rust
pub trait ParentSelector<P: OptimizationProblem, Pop>: Send + Sync {
    type State: Serialize + DeserializeOwned;
    type Error: std::error::Error + Send + Sync + 'static;

    fn select_parent(
        &mut self,
        population: &Pop,
        ctx: ParentSelectionContext<'_, P>,
    ) -> Result<ParentSelection, Self::Error>;

    fn checkpoint_state(&self) -> Self::State;
    fn restore_state(&mut self, state: Self::State) -> Result<(), Self::Error>;
}

pub enum ParentSelection {
    Candidate(CandidateId),
    NoAdmissibleParent,
}
```

`NoAdmissibleParent` allows the GEPA loop to use the seed fallback explicitly.
It must not be encoded as `None` without a reason in user-visible reports.

Required behavior:

- seed is the fallback only when the population has no admissible candidate;
- selection state that affects future decisions participates in continuation;
- selection must not read validation/test feedback hidden by split policy.

### 4.3 Part Selection And Surface Mutation

GEPA mutates selected artifact parts through `EditSurface`.

Required behavior:

- selected part id and surface fingerprint are recorded as proposal context;
- parse failures become proposal failures, not silent no-ops;
- resulting proposals use normal Leaven proposal provenance and graph mutation
  through `RunContext`.

### 4.4 Acceptance And Population Updates

Acceptance compares parent and child on the sampled train cases and updates
population/frontier state only through accepted evidence.

Required trait shape:

```rust
pub trait AcceptancePolicy<P: OptimizationProblem>: Send + Sync {
    type State: Serialize + DeserializeOwned;
    type Error: std::error::Error + Send + Sync + 'static;

    fn decide(
        &mut self,
        ctx: AcceptanceContext<'_, P>,
    ) -> Result<AcceptanceDecision, Self::Error>;

    fn checkpoint_state(&self) -> Self::State;
    fn restore_state(&mut self, state: Self::State) -> Result<(), Self::Error>;
}

pub struct AcceptanceContext<'a, P: OptimizationProblem> {
    pub parent: CandidateId,
    pub child: CandidateId,
    pub batch: &'a SampledBatch,
    pub parent_assessment: AssessmentId,
    pub child_assessment: AssessmentId,
    pub graph: RunGraphView<'a, P>,
}

pub enum AcceptanceDecision {
    Accept { reason: String },
    Reject { reason: String },
    Defer { reason: String },
}
```

The initial scalar policy may implement only accept/reject, but the enum must
not prevent defer/regression-aware policies later.

Required behavior:

- acceptance sees candidate ids, assessment ids, sampled case ids, and score
  summaries;
- acceptance can reject, accept, or later grow to defer without mutating graph
  directly;
- population update records events and updates checkpointed continuation state;
- validation/test evidence does not update the train population by default.

### 4.5 Validation Policy

Validation is held out from reflection by default. Validation may inform
selection/reporting only through an explicit validation policy.

Required trait shape:

```rust
pub trait ValidationPolicy<P: OptimizationProblem>: Send + Sync {
    type State: Serialize + DeserializeOwned;
    type Error: std::error::Error + Send + Sync + 'static;

    fn after_acceptance(
        &mut self,
        ctx: ValidationContext<'_, P>,
    ) -> Result<ValidationAction, Self::Error>;

    fn checkpoint_state(&self) -> Self::State;
    fn restore_state(&mut self, state: Self::State) -> Result<(), Self::Error>;
}

pub enum ValidationAction {
    Skip,
    Evaluate {
        partition: PartitionId,
        purpose: EvaluationPurpose,
    },
}
```

Default AIME behavior:

- validation is not shown to reflection;
- validation can be reported after search and may be evaluated during search
  only if the policy explicitly schedules it;
- test is never in-loop by default.

Required behavior:

- default reflection materials exclude validation and test answers/feedback;
- final test is report-only unless an explicit policy says otherwise;
- validation cadence state is checkpointed when introduced.

### 4.6 Evaluation Cache And Best Outputs

The AIME path must enable cache behavior equivalent to GEPA's
`cache_evaluation=True`:

- repeated candidate/case/evaluator requests reuse cached evidence;
- cache keys include candidate/artifact identity, evaluator fingerprint,
  case-set version, resolved case ids, request shape, and relevant LM request
  fingerprint;
- cache hits do not charge metric calls again;
- cached search evidence is durable in the run store at the latest optimizer
  checkpoint boundary;
- final-report-only cache rows must not be persisted as resume authority unless
  the report layer introduces its own explicit resumable snapshot.

GEPA's `track_best_outputs=True` must be implemented either directly or by
Leaven's durable trace/report model. If Leaven uses the durable trace instead,
the report must expose the best case outputs and explain that this is the
Leaven equivalent.

### 4.7 GEPA Error Classes

GEPA errors must be typed before they are normalized into durable error
records:

```rust
pub enum GepaError {
    EmptyTrainPartition,
    ParentSelection(ParentSelectionError),
    BatchSampling(BatchSamplingError),
    Surface(SurfaceError),
    Reflection(ReflectionError),
    ProposalParse(ProposalParseError),
    ProposalApply(ApplyErrorRecord),
    Evaluation(EvaluationError),
    Acceptance(AcceptanceError),
    Population(PopulationError),
    Continuation(ContinuationError),
}
```

Each error must state whether graph mutation happened. If graph mutation did
not happen, durable restore must not replay or synthesize missing graph records.

## 5. Reflection Requirements

Reflection is the mutation stage. It must use Leaven LM and receive the
materials GEPA needs to improve prompts.

### 5.1 Default Prompt

The default reflection renderer must match upstream GEPA's instruction-proposal
shape closely:

- current parameter/instructions;
- selected examples and side information;
- score/feedback/trace material;
- request for a new instruction;
- fenced text replacement output.

The default parser consumes fenced text. JSON is allowed only when the caller
explicitly configures a JSON parser/prompt pair.

### 5.1.1 Reflection Request Types

```rust
pub struct ReflectionRequest<A> {
    pub parent: CandidateId,
    pub parent_artifact: A,
    pub selected_part: SurfacePartRef,
    pub sampled_batch: SampledBatch,
    pub examples: Vec<ReflectionExample>,
    pub current_instructions: String,
    pub objective: Option<String>,
}

pub struct ReflectionExample {
    pub case_id: CaseId,
    pub split: SplitRole,
    pub input: String,
    pub generated_output: String,
    pub parsed_answer: Option<String>,
    pub score: f64,
    pub feedback: String,
    pub trace_refs: Vec<TraceRef>,
    pub reference_solution: Option<String>,
    pub parse_error: Option<String>,
}

pub struct ReflectionOutput<Edit> {
    pub edit: Edit,
    pub raw_response: String,
    pub request_fingerprint: Fingerprint,
    pub response_fingerprint: Fingerprint,
    pub cost: Cost,
}
```

`ReflectionExample` is GEPA-level selected material. It is not the generic
evidence storage type. It is a projection from graph/evidence truth into a
reflection prompt.

### 5.2 Reflection Inputs

For each selected case/example, reflection input must include:

- case id and split role;
- model output answer;
- parsed score;
- natural-language feedback;
- scorer trace or raw transcript reference;
- reference solution feedback when available;
- any output parse errors.

Reflection must not receive hidden validation/test oracle material under the
default split policy.

### 5.3 LM Runtime

Reflection must call an implementation of `leaven_lm::Lm`.

Required behavior:

- async call path;
- model configurable separately from solver model;
- default model for this path is `gpt-5.4-mini` with medium reasoning effort;
- sampling/output config recorded in request metadata;
- LM cost charged to the run budget ledger under the reflection stage;
- response cache available through `leaven-lm-cache`;
- provider errors and parse errors are durable proposal failures.

### 5.4 Reflection Renderer Invariants

The default renderer must:

- include exactly one current-instruction block per selected part;
- include selected examples in stable order;
- include score and feedback adjacent to each generated output;
- preserve newlines in reference solution text;
- request fenced replacement text;
- never include hidden validation/test oracle material unless policy allows it;
- produce deterministic prompt text for identical graph/evidence inputs.

Snapshot tests must compare rendered prompt text against a checked-in fixture
derived from upstream GEPA's AIME reflection prompt shape.

## 6. AIME Task Requirements

`aime_case_report_adapter.md` owns the detailed lowering from upstream/import
records into `Case<AimeInput, AimeTarget>` envelopes, stable source-derived
`CaseId`s, and report source projection. This section remains the paper-parity
summary.

### 6.1 Dataset Materialization

The AIME materializer must produce storeable cases with:

- stable source dataset name;
- stable source row/problem id when available;
- split role;
- problem text;
- expected integer answer;
- reference solution text;
- any source metadata needed for audit.

Train, validation, and test roles must remain visible in reports and must not be
reconstructed from positional IDs alone.

Required AIME data types:

```rust
pub struct AimeDataset {
    pub train: Vec<AimeCase>,
    pub validation: Vec<AimeCase>,
    pub test: Vec<AimeCase>,
    pub fingerprint: Fingerprint,
}

pub struct AimeCase {
    pub source: AimeSource,
    pub split: SplitRole,
    pub problem: String,
    pub answer: AimeAnswer,
    pub solution: String,
    pub metadata: MetadataBag,
}

pub struct AimeSource {
    pub dataset: String,
    pub subset: Option<String>,
    pub row_id: String,
    pub revision: Option<String>,
}

pub struct AimeAnswer {
    pub integer: i64,
    pub raw: String,
}
```

`AimeCase` must be serializable by the default run store.

### 6.2 Runner Shape

The runner executes the candidate prompt against a problem. The prompt is the
artifact GEPA mutates.

Starting with the upstream GEPA seed prompt is required. It is expected and
correct that GEPA changes this prompt during optimization.

The runner must preserve:

- parsed answer text used by the scorer;
- raw provider transcript/output as trace evidence;
- LM request/response metadata;
- LM cost.

The stable runner wrapper may ask the solver to return an answer field or parse
a final answer from the raw text. That wrapper is not the optimized artifact.
For the Rust-native DSPy profile, the first solver request must match
`dspy.ChatAdapter` for `ChainOfThought(MathSolverSignature)`. If ChatAdapter
parsing fails, the wrapper must mirror DSPy's public adapter call path by
rerunning the same signature through `JSONAdapter` before treating the case as a
solver parse failure. This fallback is runner/parser behavior, not a mutated
prompt artifact.

Required runner output:

```rust
pub struct AimeRunOutput {
    pub answer_text: String,
    pub raw_output: String,
    pub trace: Vec<String>,
    pub lm_request: Option<LmRequestRecord>,
    pub lm_response: Option<LmResponseRecord>,
    pub cost: Cost,
}
```

`RunOutput` may remain the public `leaven-run` adapter type, but P8 must retain
the fields above either directly or as trace/evidence attachments. Reflection
must be able to access the generated output and feedback for selected cases.

### 6.3 Scorer Shape

The scorer must:

- parse an integer answer;
- score exact match as `1.0`, mismatch or parse failure as `0.0`;
- return feedback in GEPA/DSPy style;
- include the reference solution text in feedback when available;
- attach parse failure details when the output cannot be parsed;
- preserve feedback and trace for reflection.

Successful runner and scorer trace material must be persisted in case assessment
evidence and projected through report `trace_refs`. Empty trace refs mean no
trace was captured, not merely that the report forgot to point at stored
assessment evidence.

Required scoring result projection:

```rust
pub enum AimeScoreOutcome {
    Correct { parsed: i64 },
    Incorrect { parsed: i64, expected: i64 },
    ParseFailed { raw_output: String, expected: i64 },
}

pub struct AimeFeedback {
    pub outcome: AimeScoreOutcome,
    pub score: f64,
    pub feedback: String,
    pub reference_solution: String,
}
```

Feedback text must include enough natural language for reflection to learn from
the reference solution. Exact wording may differ from DSPy only if the P8 README
records the difference.

### 6.4 Live Provider Defaults

Live AIME run defaults:

- solver: `gpt-4.1-mini`;
- solver temperature: `1.0`;
- solver max output tokens: `32000`;
- reflector: `gpt-5.4-mini` with medium reasoning effort;
- upstream-reflector comparison override: `LEAVEN_AIME_REFLECTION_MODEL=gpt-5.1`;
- parallelism: `32`;
- search budget: `500` metric calls;
- run mode: durable;
- cache: enabled for evaluation and LM responses where configured.

## 7. Result And Report Requirements

The P8 result must expose:

- `run_id`;
- stored run / resume reference;
- stop reason;
- budget spent and cap;
- baseline train score;
- optimized train score;
- validation score;
- baseline held-out test score;
- optimized held-out test score;
- case-level scores, feedback, traces, and source ids;
- target-safe case-level baseline/optimized deltas grouped by split;
- best prompt;
- remaining parity deltas from GEPA CAIS artifact.

Absent scores must be represented as absent or errored. They must not be
reported as `0.0`.

Required report shape:

```rust
pub struct AimeParityReport {
    pub run_id: RunId,
    pub stored_run: Option<StoredRunRef>,
    pub stop_reason: StopReason,
    pub search_budget: BudgetSnapshot,
    pub report_budget: BudgetSnapshot,
    pub baseline_train: ScoreSummary,
    pub optimized_train: ScoreSummary,
    pub validation: Option<ScoreSummary>,
    pub baseline_test: Option<ScoreSummary>,
    pub optimized_test: Option<ScoreSummary>,
    pub cases: Vec<AimeCaseReport>,
    pub best_prompt: String,
    pub parity_deltas: Vec<ParityDelta>,
}

pub struct AimeCaseReport {
    pub source: AimeSource,
    pub split: SplitRole,
    pub candidate: CandidateId,
    pub score: ScoreState,
    pub feedback_ref: Option<EvidenceRef>,
    pub trace_refs: Vec<TraceRef>,
}

pub enum ScoreState {
    Present(f64),
    Absent { reason: String },
    Error { reason: String },
}
```

The public result may expose a more general Leaven report type, but it must be
able to answer every field above without reading private engine internals.

## 8. Crate Ownership

| Crate/path | Owns |
| --- | --- |
| `leaven-engine` | stopper execution, budget stop reason, durable run boundaries, cache/evidence execution, continuation enforcement. |
| `leaven-run` | ordinary durable `.run()`, explicit `.ephemeral()`, search-vs-report budget lowering, result/report facade. |
| `leaven-gepa` | GEPA loop rhythm, sampler/selector/acceptance/population/validation/reflection strategy state, GEPA continuation. |
| `leaven-lm` | provider-neutral request/response/sampling vocabulary and `Lm` trait. |
| `leaven-lm-cache` | reusable LM response caching. |
| `leaven-lm-openai` | OpenAI provider lowering, retries/backoff/rate policy, model request execution. |
| `leaven-evidence` / eval-owned crates | score, feedback, trace, case-level evidence vocabulary. |
| `examples/p8_aime_gepa` | AIME task adapter, dataset materializer, runner/scorer wiring, live smoke command. |

Forbidden edges:

- `leaven-gepa` must not depend on `leaven-lm-openai` or concrete providers.
- `leaven-engine` must not depend on GEPA-specific strategy state.
- `leaven-run` must not own GEPA loop semantics.
- P8 must not shell out to Python or bypass Leaven LM for live solver or
  reflection.

## 8.1 Module Placement Requirements

Expected implementation placement:

| Work | Placement |
| --- | --- |
| Stop context, stop decision, budget reached reason | `crates/leaven-engine/src/stage/stopper.rs`, `crates/leaven-engine/src/events.rs` |
| Search/report budget split | `crates/leaven-run/src/builder.rs` and result/report modules |
| GEPA sampler/validation/acceptance state traits | `crates/leaven-gepa/src/{sampler,validation,gate,population}.rs` or existing owning modules |
| GEPA continuation expansion | `crates/leaven-gepa/src/optimizer.rs` |
| Reflection request/example renderer | `crates/leaven-gepa/src/reflection.rs` |
| LM cache/provider wiring | `crates/leaven-lm-cache`, `crates/leaven-lm-openai`, P8 composition only |
| AIME case/source/report types | `examples/p8_aime_gepa/src/main.rs` unless reused elsewhere |
| AIME materialization script | `examples/p8_aime_gepa/scripts/materialize_hf_aime.py` |

Do not create new crates for P8-only AIME types unless a second benchmark needs
the same abstractions.

## 9. Acceptance Tests And Proofs

### 9.1 Engine / Budget

- budget stopper trips before next optimizer step;
- clean budget stop returns current best and stop reason;
- `BudgetExceeded` still refuses before mutation inside a stage;
- final report work does not consume search budget unless explicitly configured.

Minimum test cases:

1. `budget_reached_stops_without_calling_step`: optimizer step counter stays at
   zero when budget already spent equals cap.
2. `budget_reached_finishes_with_best`: previously known best is returned with
   `StopReason::BudgetReached`.
3. `budget_exceeded_inside_evaluation_is_error_guard`: evaluator tries to charge
   over cap and no assessment/proposal graph mutation is recorded.
4. `final_report_uses_report_budget`: after search budget is exhausted, final
   validation/test evaluation still runs under report budget and is tagged as
   report work.

### 9.2 GEPA

- train sampler resumes with the same next batch after restore;
- parent/part selection resumes with the same next decision after restore;
- reflection receives scorer feedback and traces from selected train cases;
- validation/test feedback is hidden from reflection by default;
- evaluation cache avoids duplicate metric charges;
- GEPA default is not one iteration.

Minimum test cases:

1. `gepa_default_runs_until_stop`: with a three-step budget, GEPA performs more
   than one step without explicit `.max_iterations(...)`.
2. `gepa_sampler_checkpoint_round_trip`: checkpoint after one batch, restore,
   and assert next sampled case ids match the uninterrupted run.
3. `gepa_parent_part_checkpoint_round_trip`: restored run chooses same parent
   and part as uninterrupted run.
4. `gepa_reflection_materials_exclude_hidden_splits`: validation/test feedback
   present in graph is absent from rendered reflection input under default
   policy.
5. `gepa_cache_hit_does_not_charge_metric_call`: repeated candidate/case
   evaluation reuses cache and budget spent remains unchanged.

### 9.3 Reflection

- rendered prompt snapshot matches upstream-style GEPA fixture;
- fenced output parses into a surface edit;
- parse failure records proposal failure;
- reflection LM cost and request metadata are charged/recorded;
- mock LM and OpenAI LM use the same `Lm` trait path.

Minimum test cases:

1. `default_renderer_matches_upstream_aime_fixture`.
2. `reflection_examples_include_score_feedback_trace_and_solution`.
3. `fenced_edit_parser_rejects_missing_fence_with_proposal_failure`.
4. `lm_reflection_records_cost_request_and_response_fingerprints`.
5. `reflection_lm_cache_reuses_identical_prompt_response`.

### 9.4 AIME

- materialized AIME cache preserves source ids and train/validation/test roles;
- deterministic smoke is labeled non-benchmark proof;
- live P8 uses `gpt-4.1-mini` solver defaults and `gpt-5.4-mini` medium-reasoning reflector;
- live P8 stops by metric-call budget and returns a stored/resumable run;
- resumed P8 does not repeat committed evaluations;
- final report includes baseline/optimized train, validation, and held-out test
  scores plus case-level feedback;
- final report case rows preserve the baseline and optimized roles even when
  the optimized best candidate is the seed and evaluation-cache hits reuse the
  same assessment rows.

Minimum test cases/smokes:

1. `aime_materializer_preserves_source_ids`.
2. `aime_scorer_feedback_contains_reference_solution`.
3. `p8_deterministic_smoke_is_ephemeral_or_labeled_non_benchmark`.
4. `p8_live_slice_uses_openai_lm_trait_for_solver_and_reflector`.
5. `p8_resume_slice_does_not_repeat_committed_case_evals`.

The live slice may use a tiny AIME subset for cost control, but it must use the
same public surfaces and provider/cache path as the full run.

### 9.5 Completion Gate

Before claiming paper-parity readiness:

- focused engine, GEPA, LM, eval, and P8 tests pass;
- `just milestone-p8` passes for deterministic smoke;
- a live P8 smoke over a bounded AIME slice runs through Leaven LM/provider
  surfaces;
- the full live AIME run can be launched durably and resumed;
- `just check` passes or any existing suite-wide SLA failure is documented as
  unrelated and separately tracked.

## 10. Non-Goals

- Exact Python API compatibility with GEPA.
- Claiming the GEPA CAIS accuracy numbers before a live run reproduces them.
- Making `leaven-gepa` provider-specific.
- Exposing graph internals to ordinary users to compensate for missing result
  ergonomics.
- Treating deterministic P8 score movement as benchmark evidence.
