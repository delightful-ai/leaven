# Leaven Eval Lowering Detail

Status: planning detail spec.
Date: 2026-05-10.

This spec defines the lowered evaluation/data/report contract used by product
builders and optimizers. It is not the public GEPA front door. The public layer
is defined in `docs/specs/gepa_public_private_surface.md`.

It is subordinate to:

- `docs/specs/initial_library.md`
- `docs/specs/guiding_principles.md`
- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`
- `docs/specs/gepa_public_private_surface.md`
- `docs/specs/gepa_optimizer_surface.md`
- `docs/specs/agentic_stage_runtime.md`
- `docs/specs/agentic_task_execution_substrate.md`
- `docs/testing/README.md`

It is also constrained by `docs/philosophy/scatter.md`: eval-related truth must
not scatter across engine, optimizer, dataset, and agentic code.

## 1. Boundary

The product distinction:

```text
User input        = train/validation/test cases, runner/scoring function/evaluator, optimizer.
Lowered eval data = dataset, splits, split-use plan, request templates, reports.
Execution         = engine evaluator calls, graph mutation, cache, budget.
Environment       = optional workspace/agent/process substrate for an evaluator.
```

`leaven-eval` owns the lowered eval data layer. It does not execute
evaluations and it is not the thing ordinary GEPA users implement.

Current inventory and move decision:

| Existing thing | Current owner | Move? | Reason |
| --- | --- | --- | --- |
| `EvaluationSet`, `EvaluationRequest`, `ResolvedEvaluationSet`, `AssessmentGranularity`, `EvaluationPurpose` | `leaven-core` | No | Cold algebra used by engine and optimizer authors without product helpers. |
| `Evaluator`, `DynEvaluator`, `EvaluationError`, evaluator registry, cache, `RunContext::evaluate` | `leaven-engine` | No | Execution authority and graph mutation stay centralized in engine. |
| `TrustPolicy`, `ReadScope`, `EvidenceVisibility` | `leaven-engine` | No | Actor/read enforcement belongs where graph views and requests are checked. |
| `CaseSet` / evaluation-set resolution | `leaven-engine` | No | Dynamic set resolution happens at run time against engine state. |
| `CasewiseEvidence`, `ScalarEvidence`, pairwise/listwise evidence | `leaven-evidence` | No | Evidence shape vocabulary is independent of lowered eval data. |
| `AgentCase`, hidden targets, workspace requirements | `leaven-agentic` | No | Agentic task/environment semantics are domain/runtime-shaped. |

`leaven-eval` fills the gap between the product builder and the engine. It owns:

- `Dataset` and case ids;
- `DatasetSplits` and train/validation/test roles;
- split-use rules that say which split may drive optimization decisions;
- final-test defaults;
- request templates for optimizers/builders;
- report schemas that cite graph ids and evidence refs.

`leaven-eval` does not own:

- public builder verbs such as `.train`, `.validation`, `.test`, `.score`;
- evaluator execution;
- workspaces, sandboxes, processes, or agent sessions;
- optimizer rhythm, strategy state, parent selection, part selection, or
  acceptance policy;
- domain case semantics.

## 2. Crate And Module Graph

### 2.1 Crate Dependencies

`leaven-eval` may depend on:

```text
leaven-kernel
leaven-core
```

`leaven-eval` must not depend on:

```text
leaven-engine
leaven-run
leaven-gepa
leaven-mipro
leaven-textgrad
leaven-trace
leaven-agent
leaven-agentic
leaven-workspace
leaven-workspace-*
leaven-lm-*
leaven-dsrs
leaven-cuda
leaven-python
concrete provider/runtime crates
```

`leaven-run` and optimizer crates may depend on `leaven-eval`. The dependency
does not go the other way.

### 2.2 Module Graph

Planned `crates/leaven-eval/src/lib.rs` map:

```text
dataset.rs      Case, LmCase, Dataset, DatasetBuilder
error.rs        DatasetError, DatasetSplitsError, EvaluationPlanError, ReportError
plan.rs         EvaluationPlan, EvaluationPlanId, EvaluationRequestShape
report.rs       EvaluationReport, SplitReport, CandidateEvaluationSummary, SplitUseSummary
request.rs      EvaluationRequestTemplate
split.rs        DatasetSplits, SplitRole, SplitPolicy
use_policy.rs   SplitUsePolicy, SplitUse, EvaluationUse, FinalTestPolicy
traits.rs       IntoEvaluationSuite
suite.rs        EvaluationSuite
```

`lib.rs` remains a map plus curated re-exports.

### 2.3 Public Re-Exports

The crate should expose lowered vocabulary:

```rust
pub mod dataset;
pub mod error;
pub mod plan;
pub mod report;
pub mod request;
pub mod split;
pub mod traits;
pub mod suite;
pub mod use_policy;

pub use dataset::{Case, Dataset, DatasetBuilder, LmCase};
pub use error::{DatasetError, DatasetSplitsError, EvaluationPlanError, ReportError};
pub use plan::{EvaluationPlan, EvaluationPlanId, EvaluationRequestShape};
pub use report::{
    CandidateEvaluationSummary, EvaluationReport, SplitReport, SplitUseSummary,
};
pub use request::EvaluationRequestTemplate;
pub use split::{DatasetSplits, SplitPolicy, SplitRole};
pub use suite::EvaluationSuite;
pub use traits::IntoEvaluationSuite;
pub use use_policy::{EvaluationUse, FinalTestPolicy, SplitUse, SplitUsePolicy};
```

Do not expose a facade trait that hides engine execution. If users need to run
an eval directly, they install an `Evaluator<P>` into the engine or use
`leaven-run` product helpers.

## 3. Core Types

### 3.1 Split Roles

```rust
pub enum SplitRole {
    Train,
    Validation,
    Test,
    Search,
    Probe,
    ReportOnly,
    Custom(smol_str::SmolStr),
}
```

`PartitionId` remains the `leaven-core` type. `SplitRole` owns conventional
train/validation/test/search/probe/report-only meaning for lowered eval data.

### 3.2 Dataset Splits

```rust
pub struct DatasetSplits {
    version: CaseSetVersion,
    roles: BTreeMap<PartitionId, SplitRole>,
    cases: BTreeMap<PartitionId, Vec<CaseId>>,
    policy: SplitPolicy,
    fingerprint: Fingerprint,
}

pub enum SplitPolicy {
    DisjointRequired,
    OverlapAllowed { reason: String },
}
```

Construction is fallible:

```rust
impl DatasetSplits {
    pub fn new(
        version: CaseSetVersion,
        roles: BTreeMap<PartitionId, SplitRole>,
        cases: BTreeMap<PartitionId, Vec<CaseId>>,
        policy: SplitPolicy,
    ) -> Result<Self, DatasetSplitsError>;

    pub fn role(&self, partition: &PartitionId) -> Option<&SplitRole>;
    pub fn cases(&self, partition: &PartitionId) -> Option<&[CaseId]>;
    pub fn fingerprint(&self) -> Fingerprint;
    pub fn version(&self) -> &CaseSetVersion;
}
```

`DatasetSplits` is authoritative for split membership in `leaven-eval`
reports. `RunContext` still resolves `EvaluationSet` before evaluator calls.

### 3.3 Dataset

```rust
pub struct Dataset<C = Case> {
    cases: BTreeMap<CaseId, C>,
    fingerprint: Fingerprint,
    metadata: MetadataBag,
}

pub enum NoTarget {}

pub struct Case<I = serde_json::Value, T = NoTarget> {
    pub id: CaseId,
    pub input: I,
    pub target: Option<T>,
    pub metadata: MetadataBag,
}

pub type JsonCase = Case<serde_json::Value, serde_json::Value>;
pub type LmCase<I = serde_json::Value, T = serde_json::Value> = Case<I, T>;
```

Datasets are optional. Single-task search, live human evals, scalar score
functions, and online pairwise tournaments may have no stable dataset and still
use engine evaluation.

A dataset case means "unit of work", not "labeled example". `target` is
optional by design. Fixed references, hidden verifier targets, LLM judges,
human judgments, environment score, and open-ended task scoring all lower
through scorer/evaluator execution without making gold labels a dataset
requirement.

Agentic case suites do not move into `leaven-eval`. They may lower case ids and
split roles into this crate while keeping hidden targets and workspace
requirements in `leaven-agentic`.

### 3.4 Evaluation Request Shape

```rust
pub enum EvaluationRequestShape {
    Independent,
    Pairwise { order: PairOrder },
    Listwise,
}
```

This mirrors `leaven_core::EvaluationRequest` shape. It is a template field,
not an executable request.

### 3.5 Evaluation Plan

`EvaluationPlan` is lowered configuration. It is not the public concept users
must learn to run GEPA.

```rust
pub struct EvaluationPlan {
    pub id: EvaluationPlanId,
    pub request_shape: EvaluationRequestShape,
    pub granularity: AssessmentGranularity,
    pub split_use: SplitUsePolicy,
    pub report_metrics: Vec<MetricAxis>,
    pub metadata: MetadataBag,
}
```

`EvaluationPlan` must not:

- hold an `Evaluator<P>`;
- execute an evaluator;
- allocate workspaces;
- mutate a run graph;
- choose optimizer parents or parts;
- know GEPA/MIPRO/TextGrad/Trace strategy state.

### 3.6 Evaluation Suite

```rust
pub struct EvaluationSuite<C = Case> {
    pub plan: EvaluationPlan,
    pub dataset: Option<Dataset<C>>,
    pub splits: Option<DatasetSplits>,
    pub fingerprint: Fingerprint,
    pub metadata: MetadataBag,
}
```

`EvaluationSuite` means "lowered plan plus optional data and split metadata".
It does not mean "runnable evaluator".

### 3.7 Metric Axis

```rust
pub struct MetricAxis {
    pub id: smol_str::SmolStr,
    pub direction: Option<ScoreDirection>,
    pub label: Option<String>,
}

pub enum ScoreDirection {
    HigherIsBetter,
    LowerIsBetter,
}
```

`MetricAxis` is report metadata. It must not require vector-valued score
evidence before the first `leaven-eval` slice can land.

### 3.8 Score Normalization

`CandidateRunner`, `Score`, `ScoreContext`, score-on-error policy, and scoring
closure adapters belong to `leaven-run` or a domain adapter, not to
`leaven-eval`. The lowered eval layer only needs the parts that survive
execution as reportable evidence:

```text
primary comparable score
named metric axes and directions
feedback evidence references
attachment evidence references
metadata projected into reports
unscored diagnostic records
```

Score normalization must preserve information until a caller explicitly
chooses a report projection. In particular:

1. scalar scores normalize to a zero-cost `Score` with one primary score;
2. rich or `Metered<Score>` returns normalize to graph assessments, evidence
   references, and budget charges;
3. attachments are staged into the evidence/artifact store before reports cite
   them;
4. runtime paths are not durable report truth;
5. metadata cannot silently become an optimizer decision axis;
6. unscored `Score` values are valid for diagnostics and reports, but default in-loop
   GEPA policy requires at least one comparable score axis.

### 3.9 Split Use Policy

```rust
pub struct SplitUsePolicy {
    uses: BTreeMap<PartitionId, SplitUse>,
    default: SplitUse,
    final_test: FinalTestPolicy,
}

pub struct SplitUse {
    uses: BTreeSet<EvaluationUse>,
}

pub enum EvaluationUse {
    ProposerFeedback,
    ParentSelection,
    PartSelection,
    CandidateAcceptance,
    PopulationObservation,
    Report,
    EvaluatorOnly,
    FinalTest,
}

pub enum FinalTestPolicy {
    Disabled,
    FinalReportOnly,
    ExplicitlyAllowedInLoop { reason: String },
}
```

Construction is fallible. `SplitUse` must reject contradictory sets:

```text
EvaluatorOnly + ProposerFeedback
EvaluatorOnly + CandidateAcceptance
FinalTest + any in-loop use unless FinalTestPolicy explicitly allows it
empty uses for a split that appears in DatasetSplits
```

Default GEPA-compatible policy:

```text
TRAIN/SEARCH     proposer feedback, parent selection, part selection,
                 candidate acceptance, population observation, report
VALIDATION       report; optional parent/acceptance/population use only by explicit policy
TEST             final report only
PROBE            explicit probe policy only
REPORT_ONLY      report only
```

Actor/read enforcement is not modeled here. Product builders lower split-use
intent into engine `TrustPolicy`/`ReadScope`; `leaven-eval` must not import
those engine types.

### 3.10 Evaluation Request Template

```rust
pub struct EvaluationRequestTemplate {
    pub evaluator: EvaluatorId,
    pub shape: EvaluationRequestShape,
    pub set: EvaluationSet,
    pub granularity: AssessmentGranularity,
    pub purpose: EvaluationPurpose,
}

impl EvaluationRequestTemplate {
    pub fn independent(
        evaluator: EvaluatorId,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
    ) -> Self;

    pub fn build_for_candidates(
        &self,
        candidates: Vec<CandidateId>,
    ) -> Result<EvaluationRequest, EvaluationPlanError>;
}
```

This is a convenience for product builders and optimizer authors. Actual
evaluation still goes through `RunContext::evaluate` or
`RunContext::evaluate_with`.

## 4. Traits

### 4.1 `IntoEvaluationSuite`

```rust
pub trait IntoEvaluationSuite<C = Case> {
    type Error;

    fn into_evaluation_suite(
        self,
        plan: EvaluationPlan,
    ) -> Result<EvaluationSuite<C>, Self::Error>;
}
```

This adapter trait is optional and small. It must not force domain cases to
erase into `Case` when they need richer typed shape.

Expected implementations:

- `Vec<LmCase<I, T>>` in `leaven-eval`;
- `Dataset<C>` in `leaven-eval`;
- `leaven_agentic::CaseSuite` in `leaven-agentic`, not in `leaven-eval`;
- future DSRS case/program fixtures in `leaven-dsrs`, not in `leaven-eval`.

### 4.2 Deferred Engine Adapters

Closure evaluator helpers are useful, but they are not part of `leaven-eval`.
Any helper that implements `leaven_engine::Evaluator<P>` belongs in a crate that
already depends on `leaven-engine`, such as `leaven-run`, an optimizer crate, or
a later explicit adapter crate.

The first `leaven-eval` slice must land without an engine dependency.

## 5. Errors

### 5.1 `DatasetSplitsError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum DatasetSplitsError {
    #[error("dataset splits have no partitions")]
    Empty,

    #[error("partition id is empty")]
    EmptyPartitionId,

    #[error("partition `{partition}` has no role")]
    MissingRole { partition: PartitionId },

    #[error("partition `{partition}` role has no case list")]
    MissingCases { partition: PartitionId },

    #[error("partition `{partition}` contains duplicate case `{case}`")]
    DuplicateCaseInPartition { partition: PartitionId, case: CaseId },

    #[error("case `{case}` appears in multiple partitions under disjoint policy")]
    OverlapForbidden { case: CaseId, partitions: Vec<PartitionId> },

    #[error("overlap allowed policy requires a non-empty reason")]
    MissingOverlapReason,

    #[error("dataset split fingerprint failed")]
    Fingerprint { #[source] source: serde_json::Error },
}
```

### 5.2 `DatasetError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum DatasetError {
    #[error("dataset contains duplicate case `{case}`")]
    DuplicateCase { case: CaseId },

    #[error("dataset is empty")]
    Empty,

    #[error("split `{partition}` references missing case `{case}`")]
    MissingCase { partition: PartitionId, case: CaseId },

    #[error("dataset fingerprint failed")]
    Fingerprint { #[source] source: serde_json::Error },
}
```

Empty concrete datasets are rejected. No-dataset evals use `None`.

### 5.3 `EvaluationPlanError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum EvaluationPlanError {
    #[error("evaluation plan references unknown split `{partition}`")]
    UnknownSplit { partition: PartitionId },

    #[error("split `{partition}` cannot be used for `{requested}`")]
    SplitUseDenied {
        partition: PartitionId,
        requested: EvaluationUse,
    },

    #[error("test split cannot be used in-loop under final-report-only policy")]
    TestUsedInLoop { partition: PartitionId },

    #[error("request shape `{shape:?}` cannot be built for {candidate_count} candidates")]
    CandidateArity {
        shape: EvaluationRequestShape,
        candidate_count: usize,
    },

    #[error("dataset required for evaluation set `{set:?}`")]
    MissingDataset { set: EvaluationSet },
}
```

### 5.4 `ReportError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("assessment `{assessment}` references unknown request `{request}`")]
    UnknownRequest {
        assessment: AssessmentId,
        request: EvaluationRequestId,
    },

    #[error("assessment `{assessment}` belongs to unreported split `{partition}`")]
    UnreportedSplit {
        assessment: AssessmentId,
        partition: PartitionId,
    },

    #[error("report cannot choose winner because no candidates were evaluated")]
    NoEvaluatedCandidates,

    #[error("candidate `{candidate}` has no assessment for required split `{partition}`")]
    MissingRequiredSplitAssessment {
        candidate: CandidateId,
        partition: PartitionId,
    },
}
```

### 5.5 Adapter Error Mapping

Domain adapter errors should wrap, not collapse:

```text
agentic adapter missing case      -> agentic error at source, then DatasetError if lowering
DSRS evaluator runtime failure    -> DSRS evaluator error, then EvaluationError::WithSource
split policy violation            -> EvaluationPlanError or DatasetSplitsError
GEPA test use inside loop         -> GepaError/DataPolicyError wrapping EvaluationPlanError
```

## 6. Reports

### 6.1 Evaluation Report

```rust
pub struct EvaluationReport {
    pub plan_id: EvaluationPlanId,
    pub suite_fingerprint: Option<Fingerprint>,
    pub dataset_fingerprint: Option<Fingerprint>,
    pub splits: Option<DatasetSplitsSummary>,
    pub split_reports: BTreeMap<PartitionId, SplitReport>,
    pub candidate_summaries: BTreeMap<CandidateId, CandidateEvaluationSummary>,
    pub use_summary: SplitUseSummary,
}
```

`leaven-eval` defines the report schema only. Report construction from
`RunGraphView` belongs in `leaven-run`, an optimizer crate, or an engine-side
adapter that already depends on `leaven-engine`. Reports cite graph truth plus
immutable plan/dataset/split summaries; they must not duplicate artifacts or
evidence payloads.

### 6.2 Split Report

```rust
pub struct SplitReport {
    pub partition: PartitionId,
    pub role: Option<SplitRole>,
    pub use_policy: SplitUse,
    pub resolved_sets: Vec<ResolvedEvaluationSetId>,
    pub requests: Vec<EvaluationRequestId>,
    pub assessments: Vec<AssessmentId>,
    pub aggregate_score: Option<ScoreSummary>,
    pub per_case_count: usize,
}
```

### 6.3 Candidate Summary

```rust
pub struct CandidateEvaluationSummary {
    pub candidate: CandidateId,
    pub by_split: BTreeMap<PartitionId, CandidateSplitSummary>,
    pub selected_by: Option<PartitionId>,
    pub final_tested: bool,
}

pub struct CandidateSplitSummary {
    pub assessments: Vec<AssessmentId>,
    pub score: Option<ScoreSummary>,
    pub cases_observed: usize,
}

pub struct ScoreSummary {
    pub axes: BTreeMap<smol_str::SmolStr, f64>,
    pub primary: Option<f64>,
}
```

`ScoreSummary` is a report projection. It does not replace evidence records and
does not require vector evidence to be the only score shape.

## 7. Required Behavior

### 7.1 Product Builder Behavior

Product builders outside `leaven-engine` that accept `.train`, `.validation`,
`.test`, `.cases`, or `.evaluation_suite` must:

1. construct a `Dataset` when concrete cases are supplied;
2. construct `DatasetSplits` with stable `TRAIN`, `VALIDATION`, and `TEST`
   partition ids;
3. derive exactly one engine `CaseSet` from that dataset and splits before
   running;
4. treat that derived `CaseSet` as the execution source of truth for
   `RunContext` resolution;
5. keep `Dataset` and `DatasetSplits` as immutable product input/report
   fingerprints, not as a second runtime case store;
6. reject duplicate case ids in a dataset;
7. reject split references to missing cases;
8. default to `SplitPolicy::DisjointRequired`;
9. produce a deterministic fingerprint over dataset content, splits, and plan
   metadata;
10. lower split-use intent into engine `TrustPolicy`;
11. install scorer/evaluator adapters through engine builder, not through
    `EvaluationPlan`.

The engine builder may continue to accept cold `CaseSet`, evaluators,
optimizers, trust policy, and budget. It should not import `leaven-eval` just
to support product ergonomics.

Split-scoped product builders and optimizers must build train/validation/test
requests with `EvaluationSet::Partition`, not `EvaluationSet::Cases(test_ids)`,
until engine trust can map explicit case ids back to hidden partition
membership after resolution.

### 7.2 Optimizer Behavior

Optimizers using `leaven-eval` must:

1. select parents using their own strategy state;
2. build `EvaluationRequest`s from plan/template data;
3. call `RunContext::evaluate` or `RunContext::evaluate_with`;
4. respect `SplitUsePolicy` before requesting proposer feedback, parent
   selection, part selection, admission, population observation, or final
   reporting;
5. record why an evaluation ran through `EvaluationPurpose`;
6. use graph assessment IDs in reports and provenance rather than copying
   evidence payloads.

### 7.3 GEPA Behavior

GEPA must:

1. use train/search splits for default minibatch feedback;
2. hide validation/test case content from reflective proposers by default
   through builder lowering into engine trust policy;
3. admit candidates using train/search evidence unless a validation-aware
   policy is explicitly configured;
4. keep test final-report-only by default;
5. return typed errors if a policy asks for test data inside the optimization
   loop;
6. expose an evaluation report summary beside GEPA-specific candidate/frontier
   summaries.

### 7.4 Agentic Task Suite Behavior

Agentic adapters must:

1. keep `AgentCase`, hidden targets, files, setup, and workspace requirements
   in `leaven-agentic`;
2. lower case ids and partition roles into `leaven-eval`;
3. run workspaces and agents through `leaven-workspace`/`leaven-agent`;
4. record workspace/session/transcript evidence in graph assessments;
5. ensure hidden targets/test traces are scorer/evaluator-visible only;
6. use final-test reports without exposing test content to proposers.

### 7.5 DSRS/LM Program Behavior

DSRS and LM-program adapters must:

1. keep program/module artifact semantics in the domain crate;
2. use `Dataset<LmCase<...>>` or a domain dataset adapter for typed cases;
3. implement evaluator adapters that return per-case assessments when GEPA or a
   frontier optimizer needs them;
4. not require GEPA or `leaven-eval` to know DSRS program internals.

## 8. Invariants

### 8.1 Layering Invariants

- `leaven-core` types can compile without `leaven-eval`.
- `leaven-engine` can execute an `EvaluationRequest` without `leaven-eval`.
- `leaven-eval` cannot mutate `RunGraph`.
- `leaven-eval` cannot allocate workspaces or run agents.
- Optimizer crates can ignore `leaven-eval` and still implement
  `Optimizer<P>` manually.
- Domain crates may depend on `leaven-eval`, but `leaven-eval` must not depend
  back on domain crates.

### 8.2 Split Invariants

- Every partition in `DatasetSplits::cases` has exactly one `SplitRole`.
- Every partition in `DatasetSplits::roles` has a case list, unless the
  evaluation suite explicitly has no dataset.
- Under `DisjointRequired`, a `CaseId` appears in at most one partition.
- Under `OverlapAllowed`, the reason is non-empty and appears in report
  metadata.
- `TEST` is final-report-only by default.
- Split-use policy must be reflected in engine `TrustPolicy` by product
  builders.
- Split-scoped product paths must use `EvaluationSet::Partition` for
  train/validation/test requests until engine trust can map explicit case ids
  back to hidden partition membership after resolution.

### 8.3 Dataset Invariants

- `Dataset` case ids are unique.
- `Dataset` fingerprint changes when any case input, target value, metadata,
  or dataset metadata changes.
- Empty concrete datasets are rejected; no-dataset evals use `None`.
- Domain case data is preserved until the adapter consciously lowers it.

### 8.4 Request Invariants

- `EvaluationRequestTemplate` cannot build pairwise requests with a candidate
  count other than two.
- `EvaluationRequestTemplate` cannot build listwise requests with fewer than
  two candidates.
- Request `purpose` is always explicit.
- Dynamic `EvaluationSet`s are resolved only by `RunContext`, never by
  `leaven-eval`.
- `Dataset`/`DatasetSplits` are product input truth; the derived engine
  `CaseSet` is runtime resolution truth. They must be generated from the same
  input in one product-builder step.

### 8.5 Report Invariants

- Reports reference candidates, requests, resolved sets, assessments, and
  evidence by ID/ref.
- Reports do not copy artifacts.
- Reports do not expose hidden test/validation content beyond configured
  result visibility.
- Reports state whether a test score was final-report-only or explicitly used
  in-loop.

## 9. Tests

### 9.1 `leaven-eval` Law Tests

- `DatasetSplits` fingerprint is stable for identical content.
- `DatasetSplits` fingerprint changes when membership or roles change.
- `DisjointRequired` rejects overlapping case ids.
- `OverlapAllowed` requires a reason.
- `Dataset` rejects duplicate case ids.
- `Dataset` fingerprint changes when case payload changes.
- `EvaluationRequestTemplate` enforces independent/pairwise/listwise arity.
- `FinalTestPolicy::FinalReportOnly` refuses in-loop test use.

### 9.2 `leaven-eval` Example Tests

- build train/validation/test suite from three case vectors;
- build no-dataset scalar evaluation plan;
- build pairwise online evaluation plan with no dataset;
- product-builder lowering maps split use into engine `TrustPolicy`;
- produce report summary from mocked graph assessment IDs.

### 9.3 Cross-Crate Scenario Tests

Under `crates/leaven/tests` or `crates/leaven-run/tests`:

- public builder `.train/.validation/.test` yields expected splits and trust
  policy;
- GEPA default policy never evaluates test in-loop;
- validation-aware policy records validation use explicitly;
- agentic suite adapter preserves hidden target semantics and dataset splits;
- LM-program closure evaluator returns per-case assessments for train cases.

### 9.4 Topology Contract Tests

Extend `crates/leaven/tests/topology_contract.rs` so:

- `leaven-eval` may not depend on optimizer crates;
- `leaven-eval` may not depend on workspace or agentic crates;
- `leaven-core` and `leaven-engine` do not depend on `leaven-eval`;
- `leaven-run` may depend on `leaven-eval` and `leaven-engine`;
- concrete provider/runtime crates stay out of `leaven-eval` and `leaven-run`.

## 10. Implementation Order

1. Scaffold `leaven-eval` and `leaven-run` crates, add them to the workspace
   manifest, and update the topology contract expected crate set before adding
   public API.
2. Implement `split.rs` and `dataset.rs` with law tests.
3. Implement `use_policy.rs`, `plan.rs`, and `request.rs` with arity/use-policy
   tests.
4. Implement report structs as ID/ref schemas with mocked IDs.
5. Scaffold `leaven-run` builder and lower train/validation/test into
   dataset/splits/case-set/trust policy.
6. Add GEPA split-use integration over `PartitionId`s.
7. Add `CandidateRunner`, `Score`, `ScoreContext`, attachment staging, scalar
   lifting, score-on-error policy, and rich scoring closure helpers in
   `leaven-run`.
8. Add LM-program evaluator helper.
9. Add agentic adapter in `leaven-agentic`, not in `leaven-eval`.

Stop after each slice with focused tests, then run `just check` before claiming
the implementation complete.

## 11. Non-Goals

- A separate eval protocol crate.
- A new evaluator execution trait replacing `leaven_engine::Evaluator`.
- A workspace/environment abstraction.
- A benchmark catalog.
- GEPA-specific parent/part/acceptance/population policy.
- DSRS-specific program semantics.
- Agentic case/workspace ownership.
- Hidden test use for convenience.
- Compatibility aliases for old names.
