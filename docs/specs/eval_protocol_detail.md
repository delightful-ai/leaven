# Leaven Eval Protocol Detail

Status: planning detail spec.
Date: 2026-05-09.

This spec defines the concrete type-level shape for the evaluation substrate
introduced by `docs/specs/gepa_optimizer_surface.md`.

It is subordinate to:

- `docs/specs/initial_library.md`
- `docs/specs/guiding_principles.md`
- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`
- `docs/specs/gepa_optimizer_surface.md`
- `docs/specs/agentic_stage_runtime.md`
- `docs/specs/agentic_task_execution_substrate.md`
- `docs/testing/README.md`

It is also constrained by `docs/philosophy/scatter.md`: the eval substrate must
move evaluation decisions closer together, not create another place where
truth is split across engine, optimizer, dataset, and agentic code.

## 1. Boundary

The key split:

```text
Evaluation protocol  = what is measured, when, by whom, and how results count.
Dataset/case catalog = optional source of examples, tasks, prompts, fixtures, ids.
Environment          = optional execution substrate for an evaluator or agent.
```

`leaven-eval` owns declarative eval plans, optional case catalogs, split
manifests, split-use/leakage policy summaries, and graph-backed eval reports.

Current inventory and move decision:

| Existing thing | Current owner | Move? | Reason |
| --- | --- | --- | --- |
| `EvaluationSet`, `EvaluationRequest`, `ResolvedEvaluationSet`, `AssessmentGranularity`, `EvaluationPurpose` | `leaven-core` | No | Cold algebra used by engine and optimizers without product helpers. |
| `Evaluator`, `DynEvaluator`, `EvaluationError`, evaluator registry, cache, `RunContext::evaluate` | `leaven-engine` | No | Execution authority and graph mutation stay centralized in engine. |
| `TrustPolicy`, `ReadScope`, `EvidenceVisibility` | `leaven-engine` | No | Enforcement belongs where graph views and requests are checked. |
| `CaseSet` / evaluation-set resolution | `leaven-engine` | No | Dynamic set resolution happens at run time against engine state. |
| `CasewiseEvidence`, `ScalarEvidence`, pairwise/listwise evidence | `leaven-evidence` | No | Evidence shape vocabulary is independent of product eval plans. |
| `CaseSuite`, `AgentCase`, hidden targets, workspace requirements | `leaven-agentic` | No | Agentic task/environment semantics are domain/runtime-shaped. |

`leaven-eval` fills the gap between those pieces. It should not pull existing
types upward just because it references them.

`leaven-eval` does not own:

- cold evaluation algebra (`leaven-core`);
- evaluator execution, cache, registry, graph mutation, or trust checks
  (`leaven-engine`);
- workspaces, sandboxes, processes, or agent sessions (`leaven-workspace`,
  `leaven-agent`, `leaven-agentic`);
- optimizer rhythm, strategy state, candidate selection, or admission
  (`leaven-gepa`, `leaven-mipro`, `leaven-textgrad`, `leaven-trace`);
- domain case semantics (`leaven-dsrs`, `leaven-agentic`, artifact crates).

The crate exists because the following product facts otherwise scatter:

- train/validation/test meaning;
- which split may feed proposal feedback;
- which split may feed selection/admission;
- final-report-only test behavior;
- case catalog fingerprints;
- report axes and evaluator attribution;
- leakage summaries suitable for callbacks and run reports.

## 2. Crate And Module Graph

### 2.1 Crate Dependencies

`leaven-eval` may depend on:

```text
leaven-kernel
leaven-core
leaven-evidence
leaven-engine
```

`leaven-eval` must not depend on:

```text
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

`leaven-engine` remains the only crate that executes evaluations through
`Evaluator<P>` and `RunContext`. `leaven-engine` must not depend on
`leaven-eval`; product builders that combine both live in `leaven`,
`leaven-std`, or `leaven-eval` helper APIs.

### 2.2 Module Graph

Planned `crates/leaven-eval/src/lib.rs` map:

```text
case.rs        EvalCase, LmCase, CaseCatalog, CaseCatalogBuilder
error.rs       EvalPlanError, SplitManifestError, CaseCatalogError, ReportError
policy.rs      SplitUsePolicy, SplitUse, LeakagePolicy, FinalTestPolicy
protocol.rs    EvalProtocol, EvalPlan, EvalRequestTemplate, EvalRequestShape
report.rs      EvalRunReport, SplitReport, CandidateEvalSummary, EvalUseSummary
split.rs       SplitManifest, SplitRole, SplitPolicy, SplitId constants/helpers
traits.rs      IntoEvalSuite, CaseEvaluatorFn helper adapter traits only
```

`lib.rs` remains a map plus curated re-exports.

### 2.3 Public Re-Exports

The crate should expose:

```rust
pub mod case;
pub mod error;
pub mod policy;
pub mod protocol;
pub mod report;
pub mod split;
pub mod traits;

pub use case::{CaseCatalog, CaseCatalogBuilder, EvalCase, LmCase};
pub use error::{CaseCatalogError, EvalPlanError, ReportError, SplitManifestError};
pub use policy::{FinalTestPolicy, LeakagePolicy, SplitUse, SplitUsePolicy};
pub use protocol::{EvalPlan, EvalProtocol, EvalRequestShape, EvalRequestTemplate};
pub use report::{CandidateEvalSummary, EvalRunReport, EvalUseSummary, SplitReport};
pub use split::{SplitManifest, SplitPolicy, SplitRole, StandardSplit};
pub use traits::{CaseEvaluatorFn, IntoEvalSuite};
```

Do not expose a facade trait that hides engine execution. If users need to run
an eval, they still install an `Evaluator<P>` into the engine and call
`RunContext::evaluate`.

## 3. Core Types

### 3.1 Standard Splits

```rust
pub enum StandardSplit {
    Train,
    Validation,
    Test,
    Search,
    Probe,
    ReportOnly,
}

impl StandardSplit {
    pub const TRAIN: &'static str = "TRAIN";
    pub const VALIDATION: &'static str = "VALIDATION";
    pub const TEST: &'static str = "TEST";
    pub const SEARCH: &'static str = "SEARCH";
    pub const PROBE: &'static str = "PROBE";
    pub const REPORT_ONLY: &'static str = "REPORT_ONLY";

    pub fn partition_id(self) -> PartitionId;
}
```

`PartitionId` remains the `leaven-core` type. `StandardSplit` is convenience
vocabulary, not a replacement.

### 3.2 Split Manifest

```rust
pub struct SplitManifest {
    version: CaseSetVersion,
    roles: BTreeMap<PartitionId, SplitRole>,
    cases: BTreeMap<PartitionId, Vec<CaseId>>,
    policy: SplitPolicy,
    fingerprint: Fingerprint,
}

pub enum SplitRole {
    Train,
    Validation,
    Test,
    Search,
    Probe,
    ReportOnly,
    Custom(smol_str::SmolStr),
}

pub enum SplitPolicy {
    DisjointRequired,
    OverlapAllowed { reason: String },
}
```

Construction is fallible:

```rust
impl SplitManifest {
    pub fn new(
        version: CaseSetVersion,
        roles: BTreeMap<PartitionId, SplitRole>,
        cases: BTreeMap<PartitionId, Vec<CaseId>>,
        policy: SplitPolicy,
    ) -> Result<Self, SplitManifestError>;

    pub fn role(&self, partition: &PartitionId) -> Option<&SplitRole>;
    pub fn cases(&self, partition: &PartitionId) -> Option<&[CaseId]>;
    pub fn fingerprint(&self) -> Fingerprint;
    pub fn version(&self) -> &CaseSetVersion;
}
```

The manifest is authoritative for split membership within `leaven-eval`
reports. `RunContext` still resolves `EvaluationSet` before evaluator calls.

### 3.3 Case Catalog

```rust
pub struct CaseCatalog<C = EvalCase> {
    cases: BTreeMap<CaseId, C>,
    fingerprint: Fingerprint,
    metadata: MetadataBag,
}

pub struct EvalCase {
    pub id: CaseId,
    pub input: serde_json::Value,
    pub expected: Option<serde_json::Value>,
    pub metadata: MetadataBag,
}

pub struct LmCase<I = serde_json::Value, O = serde_json::Value> {
    pub id: CaseId,
    pub input: I,
    pub expected: Option<O>,
    pub metadata: MetadataBag,
}
```

Case catalogs are optional. Eval plans may exist without any catalog. A live
human eval, scalar reward, or online pairwise tournament can have no stable case
catalog and still use `EvalPlan` and `EvalRunReport`.

Agentic `CaseSuite` does not move into `leaven-eval`. It may implement
`IntoEvalSuite` or provide adapter helpers from `leaven-agentic`.

### 3.4 Eval Request Shape

```rust
pub enum EvalRequestShape {
    Independent,
    Pairwise { order: PairOrder },
    Listwise,
}
```

This mirrors `leaven_core::EvaluationRequest` shape. It is a declarative
template field, not an executable request.

### 3.5 Eval Protocol / Plan

The implementation may settle on `EvalPlan` as the public name and keep
`EvalProtocol` as an alias only before release. The durable concept is a
declarative product plan.

```rust
pub struct EvalPlan {
    pub id: EvalPlanId,
    pub request_shape: EvalRequestShape,
    pub granularity: AssessmentGranularity,
    pub split_uses: SplitUsePolicy,
    pub leakage: LeakagePolicy,
    pub report_axes: Vec<ScoreAxis>,
    pub metadata: MetadataBag,
}

pub type EvalProtocol = EvalPlan;
```

`EvalPlan` must not:

- hold an `Evaluator<P>`;
- execute an evaluator;
- allocate workspaces;
- mutate a run graph;
- choose optimizer candidates;
- know GEPA/MIPRO/TextGrad/Trace strategy state.

### 3.6 Eval Suite

```rust
pub struct EvalSuite<C = EvalCase> {
    pub plan: EvalPlan,
    pub catalog: Option<CaseCatalog<C>>,
    pub splits: Option<SplitManifest>,
    pub fingerprint: Fingerprint,
    pub metadata: MetadataBag,
}
```

`EvalSuite` means "a plan plus optional data and split metadata". It does not
mean "a runnable evaluator".

### 3.7 Split Use Policy

```rust
pub struct SplitUsePolicy {
    uses: BTreeMap<PartitionId, SplitUse>,
    default: SplitUse,
    final_test: FinalTestPolicy,
}

pub struct SplitUse {
    pub proposer_feedback: bool,
    pub optimizer_selection: bool,
    pub gate_admission: bool,
    pub population_observation: bool,
    pub report: bool,
    pub evaluator_only: bool,
}

pub enum FinalTestPolicy {
    Disabled,
    FinalReportOnly,
    ExplicitlyAllowedInLoop { reason: String },
}
```

Default GEPA-compatible policy:

```text
TRAIN/SEARCH     proposer feedback, optimizer selection, gate admission, population, report
VALIDATION       report; optional optimizer selection only by explicit policy
TEST             final report only
PROBE            explicit EvalHandle/probe policy only
REPORT_ONLY      report only
```

### 3.8 Leakage Policy

```rust
pub struct LeakagePolicy {
    hidden_from_proposers: BTreeSet<PartitionId>,
    hidden_from_optimizers: BTreeSet<PartitionId>,
    hidden_from_callbacks: BTreeSet<PartitionId>,
    proposer_evidence_visibility: EvidenceVisibility,
    callback_evidence_visibility: EvidenceVisibility,
}
```

`LeakagePolicy` lowers into `leaven_engine::TrustPolicy`. It does not replace
engine trust enforcement.

```rust
impl LeakagePolicy {
    pub fn to_trust_policy(&self) -> TrustPolicy;
}
```

If `EvidenceVisibility` remains engine-owned, `leaven-eval` may expose a small
local summary enum and map at the boundary. Do not create a duplicate trust
system.

### 3.9 Eval Request Template

```rust
pub struct EvalRequestTemplate {
    pub evaluator: EvaluatorId,
    pub shape: EvalRequestShape,
    pub set: EvaluationSet,
    pub granularity: AssessmentGranularity,
    pub purpose: EvaluationPurpose,
}

impl EvalRequestTemplate {
    pub fn independent(
        evaluator: EvaluatorId,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
    ) -> Self;

    pub fn build_for_candidates(
        &self,
        candidates: Vec<CandidateId>,
    ) -> Result<EvaluationRequest, EvalPlanError>;
}
```

This is a convenience for builders and optimizers. Actual evaluation still goes
through `RunContext::evaluate_with`.

## 4. Traits

### 4.1 `IntoEvalSuite`

```rust
pub trait IntoEvalSuite<C = EvalCase> {
    type Error;

    fn into_eval_suite(self, plan: EvalPlan) -> Result<EvalSuite<C>, Self::Error>;
}
```

This is an adapter trait for domain crates. It should stay optional and small.
It must not force domain cases to erase into `EvalCase` if they need richer
typed shape.

Expected implementations:

- `Vec<LmCase<I, O>>` in `leaven-eval`;
- `CaseCatalog<C>` in `leaven-eval`;
- `leaven_agentic::CaseSuite` in `leaven-agentic`, not in `leaven-eval`;
- future DSRs case/program fixtures in `leaven-dsrs`, not in `leaven-eval`.

### 4.2 `CaseEvaluatorFn`

Closure helpers are allowed, but only as adapters into `Evaluator<P>`.

```rust
pub trait CaseEvaluatorFn<P, C>: Send + Sync
where
    P: OptimizationProblem,
{
    type Feedback;
    type Error;

    fn evaluate_case<'a>(
        &'a self,
        artifact: &'a P::Artifact,
        case: &'a C,
    ) -> impl Future<Output = Result<CaseEvalOutcome<Self::Feedback>, Self::Error>> + Send + 'a;
}
```

Adapter:

```rust
pub struct CaseEvaluator<P, C, F> {
    id: EvaluatorId,
    fingerprint: Fingerprint,
    catalog: Arc<CaseCatalog<C>>,
    f: F,
    _marker: PhantomData<P>,
}
```

`CaseEvaluator` implements `leaven_engine::Evaluator<P>` and must:

- accept only independent requests initially;
- resolve `CaseId`s against its catalog;
- return explicit errors for missing cases;
- produce per-case assessments when requested;
- never cache by default unless the caller supplies a deterministic fingerprint
  and cache policy.

This trait lives in `leaven-eval` only if it stays generic and provider-free.
Provider/agentic evaluators live elsewhere.

## 5. Errors

### 5.1 `SplitManifestError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum SplitManifestError {
    #[error("split manifest has no partitions")]
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

    #[error("split manifest fingerprint failed")]
    Fingerprint { #[source] source: serde_json::Error },
}
```

### 5.2 `CaseCatalogError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum CaseCatalogError {
    #[error("case catalog contains duplicate case `{case}`")]
    DuplicateCase { case: CaseId },

    #[error("case catalog is empty")]
    Empty,

    #[error("split `{partition}` references missing case `{case}`")]
    MissingCase { partition: PartitionId, case: CaseId },

    #[error("case catalog fingerprint failed")]
    Fingerprint { #[source] source: serde_json::Error },
}
```

Empty catalogs may be useful for no-dataset evals, but they should be
represented as `None`, not `Some(empty_catalog)`.

### 5.3 `EvalPlanError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum EvalPlanError {
    #[error("eval plan has no report axes")]
    MissingReportAxes,

    #[error("eval plan references unknown split `{partition}`")]
    UnknownSplit { partition: PartitionId },

    #[error("split `{partition}` has no permitted use for `{requested}`")]
    SplitUseDenied {
        partition: PartitionId,
        requested: EvalUse,
    },

    #[error("test split cannot be used in-loop under final-report-only policy")]
    TestUsedInLoop { partition: PartitionId },

    #[error("request shape `{shape:?}` cannot be built for {candidate_count} candidates")]
    CandidateArity {
        shape: EvalRequestShape,
        candidate_count: usize,
    },

    #[error("case catalog required for evaluation set `{set:?}`")]
    MissingCaseCatalog { set: EvaluationSet },
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
leaven-agentic adapter missing case    -> agentic error at source, then into CaseCatalogError::MissingCase if lowering
DSRs evaluator runtime failure         -> DSRs evaluator error, then EvaluationError::WithSource
split policy violation                 -> EvalPlanError or SplitManifestError, not OptimizerError::Message
GEPA test leakage attempt              -> GepaError/DataPolicyError wrapping EvalPlanError::TestUsedInLoop
```

## 6. Reports

### 6.1 Eval Run Report

```rust
pub struct EvalRunReport {
    pub plan_id: EvalPlanId,
    pub suite_fingerprint: Option<Fingerprint>,
    pub catalog_fingerprint: Option<Fingerprint>,
    pub split_manifest: Option<SplitManifestSummary>,
    pub split_reports: BTreeMap<PartitionId, SplitReport>,
    pub candidate_summaries: BTreeMap<CandidateId, CandidateEvalSummary>,
    pub use_summary: EvalUseSummary,
    pub leakage_summary: LeakageSummary,
}
```

Reports are views over graph truth plus immutable plan/catalog/split summaries.
They must not duplicate artifacts or evidence payloads.

### 6.2 Split Report

```rust
pub struct SplitReport {
    pub partition: PartitionId,
    pub role: Option<SplitRole>,
    pub use_: SplitUse,
    pub resolved_sets: Vec<ResolvedEvaluationSetId>,
    pub requests: Vec<EvaluationRequestId>,
    pub assessments: Vec<AssessmentId>,
    pub aggregate_score: Option<ScoreVectorEvidence>,
    pub per_case_count: usize,
}
```

### 6.3 Candidate Summary

```rust
pub struct CandidateEvalSummary {
    pub candidate: CandidateId,
    pub by_split: BTreeMap<PartitionId, CandidateSplitSummary>,
    pub selected_by: Option<PartitionId>,
    pub final_tested: bool,
}

pub struct CandidateSplitSummary {
    pub assessments: Vec<AssessmentId>,
    pub score: Option<ScoreVectorEvidence>,
    pub cases_observed: usize,
}
```

## 7. Required Behavior

### 7.1 Product Builder Behavior

Product builders outside `leaven-engine` that accept `.cases`,
`.validation_cases`, `.test_cases`, or `.eval_suite` must:

1. construct a `CaseCatalog` when concrete cases are supplied;
2. construct a `SplitManifest` with stable `TRAIN`, `VALIDATION`, and `TEST`
   partition ids;
3. reject duplicate case ids in a catalog;
4. reject split references to missing cases;
5. default to `SplitPolicy::DisjointRequired`;
6. produce a deterministic fingerprint over catalog content, split manifest,
   and plan metadata;
7. lower leakage policy into engine `TrustPolicy`;
8. install evaluator(s) through engine builder, not through `EvalPlan`.

The engine builder may continue to accept cold `CaseSet`, evaluators,
optimizers, trust policy, and budget. It should not import `leaven-eval` just
to support product ergonomics.

### 7.2 Optimizer Behavior

Optimizers using `leaven-eval` must:

1. select candidates using their own strategy state;
2. build `EvaluationRequest`s from plan/template data;
3. call `RunContext::evaluate` or `RunContext::evaluate_with`;
4. respect `SplitUsePolicy` before requesting proposer feedback, admission,
   population observation, or final reporting;
5. record why an evaluation ran through `EvaluationPurpose`;
6. use graph assessment IDs in reports and provenance rather than copying
   evidence payloads.

### 7.3 GEPA Behavior

GEPA must:

1. use train/search splits for default minibatch feedback;
2. hide validation/test case content from reflective proposers by default;
3. admit candidates using train/search evidence unless a validation-aware
   policy is explicitly configured;
4. keep test final-report-only by default;
5. return typed errors if a policy asks for test data inside the optimization
   loop;
6. expose an eval report summary beside GEPA-specific candidate/frontier
   summaries.

### 7.4 MIPRO Behavior

MIPRO must:

1. use eval plan report axes for trial metrics;
2. keep surrogate/acquisition state in `leaven-mipro`;
3. use `leaven-eval` reports for train/validation/test accounting;
4. not move bootstrap or acquisition protocols into `leaven-eval`.

### 7.5 TextGrad/Trace Behavior

TextGrad and Trace-style optimizers must:

1. use `leaven-eval` for split-aware report surfaces;
2. keep gradient/critique propagation and trace credit assignment in their
   optimizer crates;
3. record trace evidence through `leaven-evidence`/graph assessment refs;
4. not require `leaven-eval` to know optimizer-specific update semantics.

### 7.6 Pairwise Behavior

Pairwise optimizers must:

1. use `EvalRequestShape::Pairwise` and `EvaluationRequest::Pairwise`;
2. record selection/eval split use in reports;
3. keep fitted preference models in `leaven-preference` or the optimizer crate;
4. allow no-catalog online pairs when cases are generated by the optimizer.

### 7.7 Agentic/Harbor/AISI Behavior

Agentic adapters must:

1. keep `AgentCase`, hidden targets, files, setup, and workspace requirements
   in `leaven-agentic`;
2. lower case ids and partition roles into `leaven-eval` manifests;
3. run workspaces and agents through `leaven-workspace`/`leaven-agent`;
4. record workspace/session/transcript evidence in graph assessments;
5. ensure hidden targets/test traces are scorer-visible only;
6. use final-test reports without exposing test content to proposers.

### 7.8 DSRs/LM Program Behavior

DSRs and LM-program adapters must:

1. keep program/module artifact semantics in the domain crate;
2. use `CaseCatalog<LmCase<...>>` or a domain catalog adapter for typed cases;
3. implement evaluator adapters that return per-case assessments when GEPA or a
   frontier optimizer needs them;
4. not require GEPA or `leaven-eval` to know DSRs program internals.

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

- Every partition in `SplitManifest::cases` has exactly one `SplitRole`.
- Every partition in `SplitManifest::roles` has a case list, unless the eval
  plan explicitly has no catalog.
- Under `DisjointRequired`, a `CaseId` appears in at most one partition.
- Under `OverlapAllowed`, the reason is non-empty and appears in report
  metadata.
- `TEST` is final-report-only by default.
- Hidden partitions in `LeakagePolicy` must be reflected in engine
  `TrustPolicy`.

### 8.3 Catalog Invariants

- `CaseCatalog` case ids are unique.
- `CaseCatalog` fingerprint changes when any case input, expected value,
  metadata, or catalog metadata changes.
- Empty concrete catalogs are rejected; no-dataset evals use `None`.
- Domain case data is preserved until the adapter consciously lowers it.

### 8.4 Request Invariants

- `EvalRequestTemplate` cannot build pairwise requests with a candidate count
  other than two.
- `EvalRequestTemplate` cannot build listwise requests with fewer than two
  candidates.
- Request `purpose` is always explicit.
- Dynamic `EvaluationSet`s are resolved only by `RunContext`, never by
  `leaven-eval`.

### 8.5 Report Invariants

- Reports reference candidates, requests, resolved sets, assessments, and
  evidence by ID/ref.
- Reports do not copy artifacts.
- Reports do not expose hidden test/validation content beyond configured
  visibility.
- Reports state whether a test score was final-report-only or explicitly used
  in-loop.

## 9. Tests

### 9.1 `leaven-eval` Law Tests

- `SplitManifest` fingerprint is stable for identical content.
- `SplitManifest` fingerprint changes when membership or roles change.
- `DisjointRequired` rejects overlapping case ids.
- `OverlapAllowed` requires a reason.
- `CaseCatalog` rejects duplicate case ids.
- `CaseCatalog` fingerprint changes when case payload changes.
- `EvalRequestTemplate` enforces independent/pairwise/listwise arity.
- `FinalTestPolicy::FinalReportOnly` refuses in-loop test use.

### 9.2 `leaven-eval` Example Tests

- build train/validation/test suite from three case vectors;
- build no-dataset scalar eval plan;
- build pairwise online eval plan with no catalog;
- lower `LeakagePolicy` into engine `TrustPolicy`;
- produce report summary from mocked graph assessment IDs.

### 9.3 Cross-Crate Scenario Tests

Under `crates/leaven/tests`:

- product builder `.cases/.validation_cases/.test_cases` yields expected split
  manifest and trust policy;
- GEPA default policy never evaluates test in-loop;
- validation-aware policy records validation use explicitly;
- agentic suite adapter preserves hidden target semantics and split manifest;
- LM-program closure evaluator returns per-case assessments for train cases.

### 9.4 Topology Contract Tests

Extend `crates/leaven/tests/topology_contract.rs` so:

- `leaven-eval` may not depend on optimizer crates;
- `leaven-eval` may not depend on workspace or agentic crates;
- `leaven-core` and `leaven-engine` do not depend on `leaven-eval` unless a
  future spec explicitly reverses this;
- concrete provider/runtime crates stay out of `leaven-eval`.

## 10. Implementation Order

1. Scaffold `leaven-eval` crate with empty modules and topology tests.
2. Implement `split.rs` and `case.rs` with law tests.
3. Implement `policy.rs` and `protocol.rs` with arity/use-policy tests.
4. Implement report structs as graph-backed summaries with mocked IDs.
5. Add product-builder lowering from cases to catalog/splits.
6. Add GEPA `GepaDataPolicy` integration over `PartitionId`s.
7. Add LM-program closure evaluator helper.
8. Add agentic adapter in `leaven-agentic`, not in `leaven-eval`.

Stop after each slice with focused tests, then run `just check` before claiming
the implementation complete.

## 11. Non-Goals

- A separate `leaven-eval-protocol` crate.
- A new evaluator execution trait replacing `leaven_engine::Evaluator`.
- A workspace/environment abstraction.
- A benchmark catalog.
- GEPA-specific selector/gate/population policy.
- DSRs-specific program semantics.
- Agentic case/workspace ownership.
- Hidden test leakage for convenience.
- Compatibility aliases for old names.
