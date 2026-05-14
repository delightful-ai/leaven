# Leaven v0.2.8 - Agentic Task Execution Substrate

> Status: pre-implementation companion spec.  
> Date: 2026-05-08.  
> Governing spec: `docs/specs/initial_library.md`.  
> Runtime companion: `docs/specs/agentic_stage_runtime.md`.  
> Stage-materialization companion: `docs/specs/agentic_stage_materialization.md`.  
> Skill companion: `docs/specs/agentic_skill_optimization_primitives.md`.  
> Purpose: define the general AISI/Inspect-inspired substrate for presenting
> tasks to agents, running agent sessions, scoring outputs, and turning those
> runs into Leaven evaluator/proposer stages.

This document is deliberately not a skill-optimization spec. Skills are one
important artifact family, but the reusable abstraction is broader:

```text
candidate artifact + task case + workspace/runtime policy
  -> materialized agent world
  -> agent session
  -> scored evidence
  -> optimizer-visible assessment
```

The AISI/Inspect lesson is the `Task` / `Sample` / `Solver` / `Scorer` /
`Sandbox` split. The Leaven version must preserve that ergonomic shape while
remaining optimizer-native:

```text
Inspect evaluates tasks over models.
Leaven evaluates tasks over evolving candidate artifacts.
```

This means the substrate must create reusable stages for GEPA and non-GEPA
optimizers. It must not become an optimizer, a provider runtime, or a
skill-specific product facade.

Concretely, the pieces worth taking from Inspect are:

- samples carry input, target, metadata, files, setup, and optional sandbox
  requirements
- task execution separates setup/presentation, agent/solver execution, scoring,
  metrics/reduction, and logging
- every sample attempt is an eventful object with transcript, tool calls, score
  events, errors, limits, retries, timings, and attachments
- sample errors have policy: retry, score anyway, fail task immediately, or
  continue and aggregate later
- sandbox/workspace cleanup is shielded so cancellation does not silently leak
  execution environments
- completed samples may be reused only when ids and dataset shape make reuse
  sound
- crashed logs are recoverable from buffered per-sample data
- tool approvals are a first-class policy and evidence surface

Leaven should not copy Inspect's mutable `TaskState` into core. The Leaven
equivalent is a durable `AgentCaseRunRecord` emitted as evidence by stage
adapters.

---

## 1. Core Cut

The general product concept is an agent workload:

```text
AgentWorkload
  cases
  case setup/files/targets/metadata
  agent presentation rules
  scorer/objective
  limits and error policy
```

This is candidate-evaluation workload vocabulary. Optimizer-stage deliberation
workspaces use `AgentStagePlan`, `AgentBacked`, `StageReadAuthority`, and
`StageAttemptReceipt` from `docs/specs/agentic_stage_materialization.md`;
`AgentCase` is not an input to those stage plans.

The optimizer consumes this through normal Leaven stages:

```text
Gepa or custom Optimizer
  -> AgentCaseEvaluator<P>    // evaluates candidate artifacts on cases
  -> AgentAuthoredProposer<P> // optionally uses an agent to author proposals
```

Provider/runtime details stay stage dependencies:

```text
CodexRuntime / ClaudeRuntime / command runtime / custom runtime
  are used by AgentCaseEvaluator or AgentAuthoredProposer
  and do not define the optimizer or artifact semantics
```

Skills enter only when the candidate artifact is a `SkillBank`, `AgentKit`, or
another artifact that contains skills. They are not the root abstraction.

---

## 2. End-User Surface

The common user should define a workload and plug the resulting evaluator into
an optimizer.

The API should support tiers:

```text
Tier 1: data-shaped workload
  user provides cases, a stock presentation preset, and a stock scorer

Tier 2: custom scoring
  user provides cases plus an AgentCaseScorer

Tier 3: custom presentation
  user controls how candidate + case become an agent workspace/request

Tier 4: custom stages/optimizer
  user drops to raw Proposer, Evaluator, Optimizer, Population pieces
```

Tier 1 should not require users to implement traits. For example:

```rust
let workload = AgentWorkload::builder()
    .cases(CaseSuite::from_jsonl("cases/train.jsonl")?)
    .validation_cases(CaseSuite::from_jsonl("cases/valid.jsonl")?)
    .presentation(AgentPresentationPreset::repo_task())
    .scorer(ScorerPreset::exact_match("answer"))
    .limits(AgentCaseLimits::default())
    .build()?;
```

Tier 3 uses the same substrate with a custom presenter:

```rust
let workload = AgentWorkload::builder()
    .cases(CaseSuite::from_jsonl("cases/train.jsonl")?)
    .validation_cases(CaseSuite::from_jsonl("cases/valid.jsonl")?)
    .present_with(MyPresentation::new())
    .score_with(MyScorer::new())
    .limits(AgentCaseLimits::default())
    .build()?;

let evaluator = AgentCaseEvaluator::builder(workload)
    .runtime(codex_runtime)
    .candidate_materializer(skill_bank_layout)
    .build()?;

let optimizer = Gepa::builder(problem)
    .surface(SkillBankSurface::default())
    .evaluator(evaluator)
    .proposer(agent_authored_skill_proposer)
    .population(ParetoFrontier::default())
    .build()?;
```

For a non-skill artifact, the same workload still applies:

```rust
let evaluator = AgentCaseEvaluator::builder(workload)
    .runtime(codex_runtime)
    .candidate_materializer(repo_layout)
    .build()?;
```

The user does not hand-roll workspace cleanup, transcript capture, budget
charging, evidence persistence, case sampling, or graph mutation. Those are
stage/product responsibilities.

---

## 3. Agent Cases

Cases are the task definition. They generalize Inspect/AISI `Sample` while
making Leaven's partitioning, cache, and trust needs explicit.

```rust
pub struct CaseSuite {
    pub cases: BTreeMap<CaseId, AgentCase>,
    pub partitions: CasePartitions,
    pub fingerprint: Fingerprint,
}

pub struct AgentCase {
    pub id: CaseId,
    pub input: CaseInput,
    pub target: CaseTarget,
    pub metadata: Metadata,
    pub files: CaseFiles,
    pub setup: Option<SetupScript>,
    pub workspace: Option<WorkspaceRequirement>,
}

pub enum CaseInput {
    Text(String),
    Messages(Vec<Message>),
    FileRef(ContentId),
    Structured(serde_json::Value),
}

pub enum CaseTarget {
    Text(String),
    Structured(serde_json::Value),
    Hidden(ContentId),
    None,
}
```

`CaseFiles` are workspace files to present for the case. `SetupScript` is a
case-local setup action run inside the workspace backend when the configured
workspace supports it.

Case laws:

- `CaseId` is stable within a suite.
- `CaseSuite.fingerprint` changes when any case input, target, metadata, file,
  setup, or partition changes.
- deterministic sampling over the same suite fingerprint, partition, and seed
  returns the same case sequence.
- hidden targets are visible to scorers but are not materialized into the
  candidate agent's workspace unless the workload explicitly makes them
  candidate-visible.
- case setup failure is recorded as case-run evidence or a structured
  evaluation error; it is not candidate artifact invalidity.
- case-local workspace requirements refine evaluator workspace selection but
  do not become candidate artifact state.

---

## 4. Presentation

Presentation is the bridge from candidate artifact plus case to an agent-ready
world. It composes materialization and rendering.

```rust
pub trait AgentCasePresenter<P>: Send + Sync
where
    P: OptimizationProblem,
{
    async fn present(
        &self,
        input: AgentCasePresentationInput<'_, P>,
        workspace: &mut WorkspaceView<'_>,
        ctx: PresentationContext<'_, P>,
    ) -> Result<Metered<AgentCasePresentation>, PresentationError>;
}

pub struct AgentCasePresentationInput<'a, P>
where
    P: OptimizationProblem,
{
    pub candidate: &'a P::Artifact,
    pub case: &'a AgentCase,
    pub graph: RunGraphView<'a, P>,
}

pub struct AgentCasePresentation {
    pub request: AgentRunRequest,
    pub output_contract: OutputContract,
    pub materialized_refs: Vec<MaterializedRef>,
}
```

The presenter may:

- write candidate artifacts into the workspace
- write case input/files/setup into the workspace
- build instructions or config for the runtime
- choose output contracts
- include visible history permitted by trust/read scope

The presenter must not:

- mutate the run graph
- run the agent
- score the result
- materialize hidden partitions or hidden targets unless explicitly authorized

Presentation laws:

- same candidate, case, presenter fingerprint, workspace capability set, and
  visible graph snapshot produce the same workspace bytes and run request
  modulo declared nondeterminism.
- presentation cost is metered separately from runtime cost.
- presentation failure preserves candidate id, case id, and the failing path or
  materialization step where known.

---

## 5. Agent Case Evaluation

`AgentCaseEvaluator<P>` is the stock evaluator adapter. It is the Leaven
analogue of Inspect's task execution loop, but it evaluates evolving candidate
artifacts rather than a fixed model.

```text
candidate + case
  -> allocate workspace
  -> presenter writes candidate/case world
  -> AgentRuntime runs one session
  -> collect output files/session transcript
  -> scorer returns evidence
  -> evaluator returns Assessment<P>
```

Shape:

```rust
pub struct AgentCaseEvaluator<P>
where
    P: OptimizationProblem,
{
    pub cases: CaseSuite,
    pub sampler: Box<dyn DynCaseSampler>,
    pub presenter: Box<dyn AgentCasePresenter<P>>,
    pub runtime: Arc<dyn AgentRuntime>,
    pub provider_dialect: Option<Arc<dyn AgentProviderDialect>>,
    pub scorer: Box<dyn AgentCaseScorer<P>>,
    pub workspace_factory: Arc<dyn WorkspaceFactory>,
    pub run_policy: AgentCaseRunPolicy,
}
```

`AgentCaseEvaluator` implements the existing Leaven `Evaluator<P>` stage
contract. It does not require GEPA and does not know which optimizer called it.

Evaluator laws:

- no graph mutation except through the normal returned assessment path
- every attempted case run either returns an assessment or a structured
  evaluation error according to `AgentCaseRunPolicy`
- runtime failures still preserve session/output evidence when available
- case sampling is reproducible from checkpointed sampler state
- completed assessments are cacheable only when evaluator, case suite,
  runtime, presenter, and candidate cache identities permit it

---

## 6. Run Policy, Attempts, and Recovery

Inspect's `retry_on_error`, `score_on_error`, sample-level limits, and
recoverable logs are not incidental features. Agentic workloads are expensive
and flaky; these policies need a typed home.

```rust
pub struct AgentCaseRunPolicy {
    pub retry_on_error: usize,
    pub score_on_error: bool,
    pub fail_on_error: FailOnError,
    pub max_parallel_cases: Option<NonZeroUsize>,
    pub max_parallel_workspaces: Option<NonZeroUsize>,
    pub limits: AgentCaseLimits,
    pub approval: Option<ToolApprovalPolicy>,
    pub checkpoint: CaseCheckpointPolicy,
}

pub enum FailOnError {
    Any,
    Never,
    Count(NonZeroUsize),
    Fraction(FiniteRatio),
}

pub struct AgentCaseLimits {
    pub message_limit: Option<NonZeroUsize>,
    pub token_limit: Option<NonZeroUsize>,
    pub time_limit: Option<Duration>,
    pub working_time_limit: Option<Duration>,
    pub cost_limit: Option<Cost>,
}
```

Attempt policy is separate from proposal repair:

```text
case retry:
  same candidate, same case, same evaluator
  used for runtime/transient/sample execution failures
  produces AgentCaseAttempt records

proposal repair:
  same proposer authors a revised proposal before graph admission
  used for parse/validation failures in proposed artifacts
  produces repair-attempt records
```

They must not be collapsed. A bad candidate failing a task is evaluator
evidence. An invalid proposed artifact is proposer repair feedback.

### 6.1 Case run records

The durable unit for an agentic evaluator is a case run record:

```rust
pub struct AgentCaseRunRecord {
    pub run_id: RunId,
    pub candidate: CandidateId,
    pub case: CaseId,
    pub partition: EvaluationSetId,
    pub attempt: NonZeroUsize,
    pub presentation: PresentationRef,
    pub session: AgentSessionRef,
    pub provider_events: Vec<AgentProviderEvent>,
    pub outputs: WorkspaceOutputsRef,
    pub score: Option<EvidenceRef>,
    pub error: Option<AgentCaseRunError>,
    pub limits: Vec<CaseLimitHit>,
    pub retries: Vec<AgentCaseRetryRecord>,
    pub timing: CaseTiming,
    pub cost: Cost,
}
```

The evaluator may lower this record into `P::Evidence`, but the record should
remain available for audit, recovery, and downstream selectors.

Record laws:

- every attempted case run has a record, even if scoring fails
- retries keep the same logical case/candidate ids and distinct attempt ids
- runtime/session evidence is retained when available, even on failure
- score-on-error records both the execution error and the scorer output
- limit hits are not represented as generic runtime failure strings
- records are checkpointed before they are used to update population state

### 6.2 Reuse and resume

Completed case runs may be reused only when all relevant identities match:

```text
candidate cache identity
case suite fingerprint
case id and partition
evaluator/scorer fingerprint
presenter/materializer fingerprint
runtime fingerprint
provider dialect fingerprint
visible trust scope
```

Runs with errors, invalidation markers, incomplete sessions, or abandoned
workspaces are not reused as completed assessments. They may be recovered as
diagnostic evidence.

Restore laws:

- completed case records are not charged twice
- completed case records are not rerun unless invalidated by identity mismatch
- in-flight workspaces are abandoned or janitored, not resumed as if complete
- sampler position and retry counts are restored explicitly
- recovery may reconstruct partial evidence, but partial evidence does not
  become a completed assessment without scorer/evaluator confirmation

---

## 7. Scoring

Scoring is user-owned task semantics. Leaven supplies the decision surface and
storage discipline.

```rust
pub trait AgentCaseScorer<P>: Send + Sync
where
    P: OptimizationProblem,
{
    async fn score(
        &self,
        input: AgentCaseScoreInput<'_, P>,
    ) -> Result<Metered<P::Evidence>, AgentScoringError>;
}

pub struct AgentCaseScoreInput<'a, P>
where
    P: OptimizationProblem,
{
    pub candidate: &'a P::Artifact,
    pub case: &'a AgentCase,
    pub session: &'a AgentSession,
    pub outputs: &'a WorkspaceOutputs,
    pub provider_events: &'a [AgentProviderEvent],
}
```

Scoring laws:

- finite numeric scores stay finite; no NaN or infinity sentinels
- low quality output is evidence, not artifact invalidity
- missing expected output is a scoring/runtime evidence condition unless the
  presenter output contract says the run is invalid
- scorer failures are structured by caller decision: retry, mark case failed,
  abort evaluation, or record unscored evidence
- scorer output must be persisted enough that selectors, populations,
  reflectors, and audits do not need to rerun the agent to understand the
  decision

---

## 8. Provider Dialects

`AgentRuntime` runs one session. It should not know candidates, cases,
optimizers, GEPA, or skills.

Provider dialects interpret provider-specific event streams after a session:

```rust
pub trait AgentProviderDialect: Send + Sync {
    fn id(&self) -> AgentProviderDialectId;
    fn fingerprint(&self) -> Fingerprint;

    fn parse_events(
        &self,
        session: &AgentSession,
    ) -> Result<Vec<AgentProviderEvent>, AgentProviderDialectError>;
}

pub enum AgentProviderEvent {
    Message { role: AgentRole, span: TraceRef },
    ToolCall { name: ToolName, args: Metadata, span: TraceRef },
    Command { record: CommandRecord },
    FileRead { path: WorkspacePath, span: Option<TraceRef> },
    FileWrite { path: WorkspacePath, span: Option<TraceRef> },
    ProviderSpecific { kind: String, data: Metadata, span: Option<TraceRef> },
}
```

Skill activation is an optional dialect overlay, not the general abstraction:

```rust
pub trait SkillEventDialect: AgentProviderDialect {
    fn parse_skill_events(
        &self,
        session: &AgentSession,
        provider_events: &[AgentProviderEvent],
    ) -> Result<Vec<SkillUseEvent>, SkillDialectError>;
}
```

This admits the unpleasant truth: every agent product has its own skill tool,
skill layout, and transcript conventions. Leaven normalizes what it can, keeps
raw trace refs, and treats absence of parsed telemetry as unknown.

Dialect laws:

- raw provider output is preserved before normalization
- parser output is deterministic for the same raw session and dialect version
- "not parsed" is not "did not happen"
- provider-specific interpretation errors do not become candidate artifact
  validation errors

---

## 9. Tool Approval and Trust

Inspect treats tool approval as a first-class decision with outcomes like
approve, modify, reject, terminate, and escalate. Leaven needs the same concept
at the runtime/stage boundary, not hidden inside provider-specific code.

```rust
pub enum ToolApprovalDecision {
    Approve,
    Modify { replacement: ToolCall },
    Reject { reason: DiagnosticText },
    Terminate { reason: DiagnosticText },
    Escalate { reason: DiagnosticText },
}

pub struct ToolApprovalRecord {
    pub call: ToolCall,
    pub decision: ToolApprovalDecision,
    pub approver: ApproverId,
    pub metadata: Metadata,
    pub trace: Option<TraceRef>,
}
```

Approval policy belongs with stage/runtime configuration and trust policy. It
is not an artifact property unless the candidate being optimized is itself a
tool policy or agent manifest.

Approval laws:

- every modified/rejected/terminated tool call is preserved in evidence
- termination is a case-run outcome, not a candidate graph mutation
- approval decisions participate in evaluator/runtime fingerprinting when they
  can affect observed behavior
- provider adapters may offer native approval hooks, but normalized evidence
  must still be available to Leaven stages

---

## 10. Agent-Authored Proposals

The same substrate also supports proposers that run agents to author changes.
This is separate from evaluation.

```text
proposer request
  -> allocate workspace
  -> present candidate/context
  -> run authoring agent
  -> parse workspace/session output into ProposalBatch<P>
  -> local apply/validate
  -> bounded repair loop if invalid
  -> return ProposalBatch<P>
```

`AgentAuthoredProposer<P>` should live in `leaven-agentic`. Skill mutation is a
specific parser/materializer configuration over `SkillBank`; codebase mutation
or harness mutation use the same proposer skeleton with different artifact
parsers.

Repair laws:

- repair is proposer-owned, not engine-owned
- the same configured proposer receives typed parse/validation feedback
- provider thread continuation is optional; portable semantics are explicit
  prior context plus bounded attempts
- invalid proposals never enter the graph
- failed attempts are persisted as stage events/evidence with cost

---

## 11. GEPA Integration

GEPA should consume the general stage adapters exactly like any other
optimizer.

Skill learning through GEPA is:

```text
P::Artifact = SkillBank or AgentKit
S = SkillBankSurface or AgentKitSurface
Evaluator = AgentCaseEvaluator<P>
Proposer = AgentAuthoredProposer<P> configured with a skill mutation parser
PartSelector = skill/file selector, optionally trace-aware
Population = Pareto/frontier implementation
Runtime = stage dependency
```

The important rule:

```text
provider runtime is not part of the optimizer name
skill artifact is not part of the optimizer name
agent workload is not part of the optimizer name
```

GEPA remains GEPA. The agentic task substrate gives it evaluators/proposers.

---

## 12. Crate Placement

Likely ownership:

```text
leaven-agent
  AgentRuntime, AgentRunRequest, AgentSession, OutputContract

leaven-agentic
  AgentWorkload, AgentCaseEvaluator, AgentCasePresenter,
  AgentCaseScorer, AgentAuthoredProposer, provider dialect traits

leaven-artifact-skill
  SkillBank, SkillFolder, SKILL.md validation, skill surfaces

leaven-agentic-skill
  skill-specific presenters, proposal parsers, skill-event dialect overlays,
  and convenience constructors over leaven-agentic

leaven-gepa
  GEPA rhythm and GEPA-specific strategy wiring only
```

`leaven-agentic` may depend on `leaven-agent`, `leaven-workspace`,
`leaven-engine`, `leaven-core`, and `leaven-kernel`. It must not depend on
Codex, Claude, specific skill artifacts, or GEPA.

Provider crates implement runtimes and dialects. Skill crates implement skill
artifacts and skill-specific adapters.

---

## 13. Test Contract

General case/evaluator tests:

- `CaseSuite` fingerprint changes on input, target, metadata, files, setup,
  and partition changes.
- deterministic sampler resumes the same sequence after checkpoint restore.
- hidden targets are not materialized by default.
- case-local workspace requirements refine workspace selection without changing
  candidate artifact identity.
- presenter writes expected workspace files without graph mutation.
- runtime failure returns/preserves available session evidence.
- scorer failure preserves case id, candidate id where known, and retryability.
- score-on-error records both the runtime error and scorer output.
- completed case reuse fails closed on identity mismatch.
- recovered partial records do not become completed assessments by accident.
- `AgentCaseEvaluator` can be used by GEPA and a non-GEPA optimizer without
  depending on either crate.

Provider dialect tests:

- raw provider session is preserved before event parsing.
- dialect parser is deterministic.
- unsupported event shapes produce structured dialect errors.
- missing parsed skill telemetry is represented as unknown, not false.

Proposer tests:

- invalid parsed proposal enters the bounded repair loop.
- repair attempts are costed and persisted.
- locally valid proposals still go through `RunContext` for final admission.
- graph admission failure does not secretly call the provider again.

Approval/trust tests:

- approve/modify/reject/terminate decisions are persisted as case-run evidence.
- modified tool calls preserve both original and replacement calls.
- terminated case runs do not mutate the candidate graph.
- approval policy fingerprint changes cache identity for affected evaluations.

Skill-specific tests belong in the skill companion spec and should reuse these
general contract suites rather than duplicating fake local agent-task types.

---

## 14. Short Form

```text
AgentWorkload = cases + presentation + scorer + limits.
AgentCase = input, target, metadata, files, setup, workspace requirement.
AgentCaseEvaluator = stock Evaluator<P> over candidate artifacts and cases.
AgentCasePresenter = candidate + case -> workspace + AgentRunRequest.
AgentRuntime = one session, no optimizer knowledge.
AgentProviderDialect = provider transcript/event normalization.
SkillEventDialect = optional overlay for skill-use telemetry.
AgentCaseScorer = user-owned task semantics -> P::Evidence.
AgentCaseRunPolicy = retries, score-on-error, fail-on-error, limits, approval.
AgentCaseRunRecord = durable attempted case execution evidence.
AgentAuthoredProposer = stock proposer skeleton for agent-authored changes.
GEPA consumes these as stages; it is not renamed.
Skills specialize the artifact/presenter/parser/evidence pieces.
```
