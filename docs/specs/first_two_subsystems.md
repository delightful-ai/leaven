> Status: superseded by `docs/specs/initial_library.md` v0.2.2 and
> `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`.
> Do not implement from this file without first updating it to the current
> artifact/cache identity split, `EditSurface` topology, `Materializer`
> vocabulary, `WorkspacePath` workspace API, and GEPA selector/population split.

Below is a v0.2.1-oriented historical “almost implementation” sketch for the two load-bearing subsystems:

1. **Proposal / Candidate / RunGraph data model**
2. **RunContext services**

I’m treating v0.2.1 as the tightened version of v0.2 with these corrections baked in:

```text
ProposalEffect::Create | Change
ProposalProvenance { causal, informed_by }
per-proposal provenance
Proposer::Request associated type
RunContext is the only graph-writing surface
Workspace/rendering exists but stays outside this specific core pass
Content-addressing is a capability, not a lie every artifact must tell
```

The design goal is: the types should preserve causal truth, prevent accidental information loss, and make graph-writing, budget-charging, cache, events, and trust boundaries hard to bypass. That follows the type-design rule that information should hold its shape until consciously reshaped, and that time/causality should be explicit in types.  The API should also minimize scatter, implicit conventions, lies, drift, translation, and noise, so optimizer authors navigate by concepts rather than storage mechanics.  Traits below are kept cold only where the capability is real and independently useful; anything GEPA-specific stays warm.

---

# 0. Crate and Module Graph

I would split the project into crates like this:

```text
optimize-core
  src/
    ids.rs
    time.rs
    metadata.rs
    cost.rs
    error.rs

    artifact.rs
    problem.rs

    candidate.rs
    proposal.rs
    evaluation.rs
    evidence.rs
    preference.rs
    population.rs

    graph/
      mod.rs
      storage.rs
      view.rs
      indices.rs
      query.rs
      events.rs

    context/
      mod.rs
      run_context.rs
      proposal_context.rs
      evaluation_context.rs
      trust.rs

    stage/
      mod.rs
      evaluator.rs
      proposer.rs
      renderer.rs
      callback.rs
      stopper.rs

    engine.rs
    result.rs
    prelude.rs

optimize-runtime
  Optional heavier runtime helpers:
    cache.rs
    stores/
      inline_serde.rs
      file.rs
      sqlite.rs
      object_store.rs
    workspace/
      local.rs
      docker.rs
      e2b.rs
      git_worktree.rs

optimize-derive
  #[derive(Optimize)]
  #[derive(ContentAddressed)]

optimize-std
  Standard artifact/evidence/preference/population impls:
    artifacts/
    evidence/
    preferences/
    populations/

optimize-gepa
  Gepa optimizer and GEPA-specific warm traits:
    Gepa
    ReflectiveMutation
    SystemAwareMerge
    CandidateSelector
    PartSelector
    BatchSampler
    Acceptance
    ValidationPolicy

optimize-agent
  Agent runtime helpers, repo task evaluators, skill-kit helpers.

optimize-dsrs
  DSRs/DSPy integration.
```

Dependency direction:

```text
optimize-core
  ↑
optimize-runtime
  ↑
optimize-std
  ↑
optimize-gepa / optimize-agent / optimize-dsrs
```

`optimize-core` should not depend on GEPA, agent code, LLM SDKs, or workspace backends. It defines the algebra.

---

# 1. Subsystem: Proposal / Candidate / RunGraph

## 1.1 Module layout

```text
optimize-core/src/
  ids.rs
  metadata.rs
  error.rs
  artifact.rs
  problem.rs
  candidate.rs
  proposal.rs
  evaluation.rs
  evidence.rs
  graph/
    mod.rs
    storage.rs
    view.rs
    indices.rs
    query.rs
    events.rs
```

---

# 2. IDs

```rust
// ids.rs

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct RunId(pub uuid::Uuid);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct CandidateId(pub uuid::Uuid);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ProposalBatchId(pub uuid::Uuid);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ProposalId(pub uuid::Uuid);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ApplyAttemptId(pub uuid::Uuid);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct EvaluationRequestId(pub uuid::Uuid);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct AssessmentId(pub uuid::Uuid);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct PopulationId(pub uuid::Uuid);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct StageId(pub uuid::Uuid);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct IterationId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ContentId(pub [u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ArtifactId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ArtifactIdentity {
    /// Collision-resistant content identity.
    Content(ContentId),

    /// Stable identity, but not safe for deterministic dedup/cache by itself.
    External(ArtifactId),
}
```

## Identity law

```text
CandidateId is graph-local.
ArtifactIdentity identifies artifact state.
ContentId, when present, means deterministic cache/dedup may treat equal content as equal.
External identity is not enough for deterministic eval caching unless the user supplies a cache key.
```

This avoids forcing every artifact to pretend to be fully content-addressed.

---

# 3. Artifact

```rust
// artifact.rs

pub trait Artifact: Clone + Send + Sync + 'static {
    type Change: Clone + Send + Sync + 'static;
    type Error: std::error::Error + Send + Sync + 'static;

    /// Stable identity of this artifact state.
    fn identity(&self) -> ArtifactIdentity;

    /// Optional validation for newly-created artifacts.
    ///
    /// Default is valid-by-construction. Domain artifacts with syntactic or interface
    /// requirements should override this or make invalid values unconstructible.
    fn validate(&self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Apply a change to this artifact.
    ///
    /// Must be functional. Same artifact + same change either fails the same way
    /// or produces the same identity.
    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::Error>;
}

/// Stronger identity capability.
pub trait ContentAddressed: Artifact {
    fn content_id(&self) -> ContentId;
}
```

## Artifact laws

```text
validate is pure.
apply_change is pure.
failed apply_change does not mutate the original artifact.
Artifact::identity must include every state component visible to library operations.
ContentAddressed::content_id must be collision-resistant over all observationally relevant state.
```

For default derived artifacts, `#[derive(Optimize)]` should generate canonical serialization and a cryptographic content hash.

---

# 4. OptimizationProblem

```rust
// problem.rs

pub trait OptimizationProblem: Send + Sync + 'static {
    type Artifact: Artifact;
    type Case: Send + Sync + 'static;
    type Evidence: Evidence;
    type ProposalAnnotations: Clone + Send + Sync + 'static;
}
```

Run-wide evidence/annotation enums are intentional:

```rust
pub enum MyEvidence {
    Scalar(ScalarEvidence),
    Pairwise(PairwiseJudgmentEvidence),
    AgentTrace(AgentTrajectoryEvidence),
}

pub enum MyProposalAnnotations {
    None,
    Reflection(ReflectionAnnotations),
    Edit(EditAnnotations),
    Merge(MergeAnnotations),
}
```

This keeps the run honest about all shapes that may occur.

---

# 5. Candidate

```rust
// candidate.rs

#[derive(Clone, Debug)]
pub struct Candidate<P: OptimizationProblem> {
    pub id: CandidateId,
    pub identity: ArtifactIdentity,
    pub artifact: P::Artifact,
    pub origin: CandidateOrigin,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug)]
pub enum CandidateOrigin {
    Seed {
        seed_index: usize,
    },

    Proposal {
        proposal_id: ProposalId,
        apply_attempt_id: ApplyAttemptId,
    },
}
```

## Candidate invariants

```text
CandidateId is unique within a RunGraph.
Candidate identity is copied from artifact.identity() at creation.
Candidate origin never changes.
Multiple CandidateIds may share the same ArtifactIdentity.
Same content with different histories remains distinct candidates.
```

No candidate-level `Accepted` / `Rejected` state exists. That is population state.

---

# 6. Proposal

This is the v0.2.1 correction: **proposal effect is Create or Change.** No `Parents::None + change`.

```rust
// proposal.rs

#[derive(Clone, Debug)]
pub struct Proposal<P: OptimizationProblem> {
    pub effect: ProposalEffect<P>,
    pub provenance: ProposalProvenance,
    pub annotations: P::ProposalAnnotations,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug)]
pub enum ProposalEffect<P: OptimizationProblem> {
    /// Brand-new authored artifact.
    ///
    /// Used by Meta-Harness style optimizers, fresh program synthesis, and
    /// optimizers that do not mutate from a concrete parent.
    Create {
        artifact: P::Artifact,
    },

    /// Change one existing candidate.
    Change {
        target: CandidateId,
        change: <P::Artifact as Artifact>::Change,
    },
}

#[derive(Clone, Debug)]
pub struct ProposalProvenance {
    /// Content lineage.
    pub causal: CausalInputs,

    /// Information the proposer read or used while producing the proposal.
    ///
    /// This is not causal lineage. It is bibliographic/informational provenance.
    pub informed_by: Vec<InfoRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CausalInputs {
    None,
    Single(CandidateId),
    Pair(CandidateId, CandidateId),
    NAry(Vec<CandidateId>),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum InfoRef {
    Candidate(CandidateId),
    Proposal(ProposalId),
    ProposalBatch(ProposalBatchId),
    Assessment(AssessmentId),
    External(ExternalRef),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ExternalRef {
    pub kind: String,
    pub id: String,
}
```

## Proposal invariants

```text
ProposalEffect::Create does not call Artifact::apply_change.
ProposalEffect::Create calls artifact.validate() before candidate creation.
ProposalEffect::Change calls target_artifact.apply_change(change), then validate().
ProposalEffect::Change.target must appear in provenance.causal.
CausalInputs determines lineage.
informed_by never determines lineage.
metadata is never the source of causal or informational provenance.
```

## Merge canonicalization

For a merge:

```text
source candidates: A and B
apply target: A
imported content: extracted from B
```

Represent as:

```rust
ProposalEffect::Change {
    target: a,
    change: change_that_embeds_content_from_b,
}

ProposalProvenance {
    causal: CausalInputs::Pair(a, b),
    informed_by: vec![InfoRef::Candidate(a), InfoRef::Candidate(b)],
}
```

`Artifact::apply_change` only sees candidate A and the already-canonicalized change. The merge proposer is responsible for reading B and embedding needed content into the change.

---

# 7. ProposalBatch

```rust
// proposal.rs

#[derive(Clone, Debug)]
pub struct ProposalBatch<P: OptimizationProblem> {
    pub proposals: Vec<Proposal<P>>,
    pub semantics: ProposalBatchSemantics,
    pub metadata: MetadataBag,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalBatchSemantics {
    /// Sibling alternatives from one proposer call/context.
    Alternatives,

    /// Pool from which optimizer may apply/evaluate only a subset.
    CandidatePool,
}
```

I would not include `Ordered` until a prototype proves it. Ordered dependencies should normally be expressed by multiple optimizer steps.

## ProposalBatch invariants

```text
Batch groups proposals from one stage call.
Batch does not own parents.
Each proposal owns its effect and provenance.
Alternatives are independent if applied.
CandidatePool may be partially applied/evaluated.
Batch creation cost is charged at stage level.
```

---

# 8. Metadata

```rust
// metadata.rs

#[derive(Clone, Debug, Default)]
pub struct MetadataBag {
    fields: std::collections::BTreeMap<MetadataKey, MetadataValue>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct MetadataKey(pub String);

#[derive(Clone, Debug)]
pub enum MetadataValue {
    String(String),
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    Json(serde_json::Value),
    BlobRef(BlobRef),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct BlobRef {
    pub store: String,
    pub key: String,
}
```

Metadata is for operational/debug data, not semantic graph relations.

---

# 9. Evaluation and Assessment Records

For graph completeness, define records even though the user asked mainly for proposal/candidate/graph.

```rust
// evaluation.rs

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
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

Stored graph records should use `EvidenceRef`:

```rust
#[derive(Clone, Debug)]
pub enum StoredAssessment {
    Independent {
        candidate: CandidateId,
        target: AssessmentTarget,
        evidence: EvidenceRef,
        cost: Cost,
        metadata: MetadataBag,
    },

    Pairwise {
        left: CandidateId,
        right: CandidateId,
        target: AssessmentTarget,
        evidence: EvidenceRef,
        cost: Cost,
        metadata: MetadataBag,
    },

    Listwise {
        candidates: Vec<CandidateId>,
        target: AssessmentTarget,
        evidence: EvidenceRef,
        cost: Cost,
        metadata: MetadataBag,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct EvidenceRef {
    pub store: String,
    pub key: String,
}
```

---

# 10. Error Model

The graph stores normalized errors. Runtime APIs may return typed errors, but durable graph state stores `ErrorRecord`.

```rust
// error.rs

#[derive(Clone, Debug)]
pub struct ErrorRecord {
    pub kind: ErrorKind,
    pub message: String,
    pub debug: Option<String>,
    pub source_chain: Vec<String>,
    pub retryable: Option<bool>,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    Proposal,
    Apply,
    Artifact,
    Evaluation,
    Render,
    Budget,
    Trust,
    Cache,
    Store,
    Callback,
    GraphInvariant,
    Internal,
}

pub trait IntoErrorRecord {
    fn into_error_record(self) -> ErrorRecord;
}
```

Specific apply errors:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ApplyProposalError {
    #[error("unknown candidate: {0:?}")]
    UnknownCandidate(CandidateId),

    #[error("unknown proposal: {0:?}")]
    UnknownProposal(ProposalId),

    #[error("proposal already applied: {0:?}")]
    ProposalAlreadyApplied(ProposalId),

    #[error("invalid proposal provenance: {0}")]
    InvalidProvenance(String),

    #[error("artifact apply failed")]
    Artifact {
        record: ErrorRecord,
    },

    #[error("artifact validation failed")]
    Validation {
        record: ErrorRecord,
    },

    #[error("graph invariant violation")]
    GraphInvariant {
        record: ErrorRecord,
    },
}
```

Graph-level errors:

```rust
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("candidate does not exist: {0:?}")]
    MissingCandidate(CandidateId),

    #[error("proposal does not exist: {0:?}")]
    MissingProposal(ProposalId),

    #[error("assessment does not exist: {0:?}")]
    MissingAssessment(AssessmentId),

    #[error("duplicate id: {0}")]
    DuplicateId(String),

    #[error("invariant violation: {0}")]
    Invariant(String),
}
```

## Error invariants

```text
Every failed proposal application creates an ApplyAttemptRecord.
Every fallible context method either records an Error event or returns before side effects.
No rich error is reduced to bool.
Graph stores ErrorRecord, not opaque dyn Error.
```

---

# 11. RunGraph Storage

```rust
// graph/storage.rs

pub struct RunGraph<P: OptimizationProblem> {
    pub run_id: RunId,

    candidates: indexmap::IndexMap<CandidateId, CandidateRecord<P>>,
    proposal_batches: indexmap::IndexMap<ProposalBatchId, ProposalBatchRecord>,
    proposals: indexmap::IndexMap<ProposalId, ProposalRecord<P>>,
    apply_attempts: indexmap::IndexMap<ApplyAttemptId, ApplyAttemptRecord>,

    evaluation_requests: indexmap::IndexMap<EvaluationRequestId, EvaluationRequestRecord>,
    assessments: indexmap::IndexMap<AssessmentId, AssessmentRecord>,

    population_events: Vec<PopulationEventRecord>,
    budget_events: Vec<BudgetEventRecord>,
    error_events: Vec<ErrorEventRecord>,

    events: Vec<RunEventRecord>,

    indices: GraphIndices,
}

pub struct CandidateRecord<P: OptimizationProblem> {
    pub id: CandidateId,
    pub identity: ArtifactIdentity,
    pub artifact: P::Artifact,
    pub origin: CandidateOrigin,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct ProposalBatchRecord {
    pub id: ProposalBatchId,
    pub stage: StageId,
    pub semantics: ProposalBatchSemantics,
    pub proposal_ids: Vec<ProposalId>,
    pub metadata: MetadataBag,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub iteration: Option<IterationId>,
}

pub struct ProposalRecord<P: OptimizationProblem> {
    pub id: ProposalId,
    pub batch_id: ProposalBatchId,
    pub effect: ProposalEffect<P>,
    pub provenance: ProposalProvenance,
    pub annotations: P::ProposalAnnotations,
    pub metadata: MetadataBag,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct ApplyAttemptRecord {
    pub id: ApplyAttemptId,
    pub proposal_id: ProposalId,
    pub outcome: ApplyOutcome,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub enum ApplyOutcome {
    Created {
        candidate_id: CandidateId,
        identity: ArtifactIdentity,
    },

    Failed {
        error: ErrorRecord,
    },
}

pub struct EvaluationRequestRecord {
    pub id: EvaluationRequestId,
    pub evaluator: EvaluatorId,
    pub request: ResolvedEvaluationRequest,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct AssessmentRecord {
    pub id: AssessmentId,
    pub request_id: EvaluationRequestId,
    pub evaluator: EvaluatorId,
    pub assessment: StoredAssessment,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

## Indices

```rust
pub struct GraphIndices {
    /// Content or external artifact identity -> candidates.
    pub by_identity: std::collections::HashMap<ArtifactIdentity, Vec<CandidateId>>,

    /// Candidate -> causal parents.
    pub causal_parents: std::collections::HashMap<CandidateId, Vec<CandidateId>>,

    /// Candidate -> causal children.
    pub causal_children: std::collections::HashMap<CandidateId, Vec<CandidateId>>,

    /// Candidate -> candidates/proposals/assessments that informed its proposal.
    pub informed_by: std::collections::HashMap<CandidateId, Vec<InfoRef>>,

    /// Candidate -> candidates it informed.
    pub informed: std::collections::HashMap<CandidateId, Vec<CandidateId>>,

    /// Proposal -> apply attempt.
    pub apply_by_proposal: std::collections::HashMap<ProposalId, ApplyAttemptId>,

    /// Candidate -> creating proposal.
    pub proposal_by_candidate: std::collections::HashMap<CandidateId, ProposalId>,

    /// Candidate -> assessments involving it.
    pub assessments_by_candidate: std::collections::HashMap<CandidateId, Vec<AssessmentId>>,

    /// Pair -> pairwise assessments.
    pub pairwise_assessments: std::collections::HashMap<(CandidateId, CandidateId), Vec<AssessmentId>>,
}
```

These are derived indices. The record maps are source of truth.

---

# 12. RunGraph Mutators

Graph mutation methods should be `pub(crate)`. Optimizer authors write through `RunContext`, not `RunGraph`.

```rust
impl<P: OptimizationProblem> RunGraph<P> {
    pub(crate) fn insert_seed(
        &mut self,
        artifact: P::Artifact,
        seed_index: usize,
    ) -> CandidateId {
        // validate? Seed artifacts should already be valid; still call validate for safety.
    }

    pub(crate) fn record_proposal_batch(
        &mut self,
        stage: StageId,
        batch: ProposalBatch<P>,
        iteration: Option<IterationId>,
    ) -> ProposalBatchId {
        // assign batch id
        // assign proposal ids
        // store records
        // update proposal-batch index
        // emit ProposalBatchProduced event at RunContext layer
    }

    pub(crate) fn apply_proposal_record(
        &mut self,
        proposal_id: ProposalId,
    ) -> ApplyAttemptRecord {
        // must be called only by RunContext
    }

    pub(crate) fn record_assessments(
        &mut self,
        request_id: EvaluationRequestId,
        evaluator: EvaluatorId,
        assessments: Vec<StoredAssessment>,
    ) -> Vec<AssessmentId> {
        // insert records and update assessment indices
    }

    pub(crate) fn record_population_events(
        &mut self,
        events: Vec<PopulationEvent>,
    ) {
        // append only
    }

    pub(crate) fn record_budget_event(
        &mut self,
        event: BudgetEventRecord,
    ) {
        // append only
    }

    pub(crate) fn record_error(
        &mut self,
        error: ErrorEventRecord,
    ) {
        // append only
    }
}
```

## Important implementation detail

`apply_proposal_record` should not borrow a graph view across user code. It should clone the target artifact before applying:

```rust
let target_artifact = self
    .candidates
    .get(&target)
    .ok_or(...)
    .map(|c| c.artifact.clone())?;

let new_artifact = target_artifact.apply_change(&change)?;
new_artifact.validate()?;
```

This avoids holding graph borrows across calls into user artifact code.

---

# 13. Apply Semantics

```rust
impl<P: OptimizationProblem> RunGraph<P> {
    pub(crate) fn apply_proposal_record(
        &mut self,
        proposal_id: ProposalId,
    ) -> ApplyAttemptRecord {
        let attempt_id = ApplyAttemptId(uuid::Uuid::new_v4());
        let now = chrono::Utc::now();

        let result = self.try_apply_proposal(proposal_id);

        let outcome = match result {
            Ok(candidate_id) => {
                let identity = self.candidates[&candidate_id].identity.clone();
                ApplyOutcome::Created {
                    candidate_id,
                    identity,
                }
            }
            Err(err) => ApplyOutcome::Failed {
                error: err.into_error_record(),
            },
        };

        let record = ApplyAttemptRecord {
            id: attempt_id,
            proposal_id,
            outcome,
            created_at: now,
        };

        self.apply_attempts.insert(attempt_id, record.clone());
        self.indices.apply_by_proposal.insert(proposal_id, attempt_id);

        record
    }

    fn try_apply_proposal(
        &mut self,
        proposal_id: ProposalId,
    ) -> Result<CandidateId, ApplyProposalError> {
        if self.indices.apply_by_proposal.contains_key(&proposal_id) {
            return Err(ApplyProposalError::ProposalAlreadyApplied(proposal_id));
        }

        let proposal = self
            .proposals
            .get(&proposal_id)
            .ok_or(ApplyProposalError::UnknownProposal(proposal_id))?
            .clone();

        self.validate_provenance(&proposal)?;

        let artifact = match proposal.effect {
            ProposalEffect::Create { artifact } => {
                artifact
                    .validate()
                    .map_err(|e| ApplyProposalError::Validation {
                        record: error_record_from(e),
                    })?;
                artifact
            }

            ProposalEffect::Change { target, change } => {
                let parent = self
                    .candidates
                    .get(&target)
                    .ok_or(ApplyProposalError::UnknownCandidate(target))?
                    .artifact
                    .clone();

                let child = parent
                    .apply_change(&change)
                    .map_err(|e| ApplyProposalError::Artifact {
                        record: error_record_from(e),
                    })?;

                child
                    .validate()
                    .map_err(|e| ApplyProposalError::Validation {
                        record: error_record_from(e),
                    })?;

                child
            }
        };

        let candidate_id = CandidateId(uuid::Uuid::new_v4());
        let identity = artifact.identity();

        let candidate = CandidateRecord {
            id: candidate_id,
            identity: identity.clone(),
            artifact,
            origin: CandidateOrigin::Proposal {
                proposal_id,
                apply_attempt_id: ApplyAttemptId(uuid::Uuid::nil()), // filled by caller or two-phase insert
            },
            created_at: chrono::Utc::now(),
        };

        self.candidates.insert(candidate_id, candidate);
        self.indices.by_identity.entry(identity).or_default().push(candidate_id);

        self.index_candidate_lineage(candidate_id, proposal_id, &proposal.provenance);

        Ok(candidate_id)
    }
}
```

In actual implementation, avoid the placeholder `ApplyAttemptId::nil()` by two-phase construction:

```text
1. allocate ApplyAttemptId
2. call try_apply_proposal_with_attempt_id
3. candidate origin receives real ApplyAttemptId
```

This sketch shows the flow.

## Provenance validation

```rust
fn validate_provenance<P: OptimizationProblem>(
    proposal: &ProposalRecord<P>,
) -> Result<(), ApplyProposalError> {
    match &proposal.effect {
        ProposalEffect::Create { .. } => {
            // any causal inputs allowed:
            // None for fresh authoring
            // Pair/NAry for "created from sources but not by applying to a target"
            Ok(())
        }

        ProposalEffect::Change { target, .. } => {
            if !proposal.provenance.causal.contains_candidate(*target) {
                return Err(ApplyProposalError::InvalidProvenance(
                    "Change target must appear in causal inputs".to_string(),
                ));
            }
            Ok(())
        }
    }
}
```

---

# 14. RunGraphView

Strategy authors receive views, not mutable graph internals.

```rust
// graph/view.rs

pub struct RunGraphView<'g, P: OptimizationProblem> {
    graph: &'g RunGraph<P>,
    read_scope: ReadScope,
}

impl<'g, P: OptimizationProblem> RunGraphView<'g, P> {
    pub fn candidate(
        &self,
        id: CandidateId,
    ) -> Option<CandidateView<'g, P>>;

    pub fn artifact(
        &self,
        id: CandidateId,
    ) -> Option<&'g P::Artifact>;

    pub fn identity(
        &self,
        id: CandidateId,
    ) -> Option<&'g ArtifactIdentity>;

    pub fn parents(
        &self,
        id: CandidateId,
    ) -> Vec<CandidateId>;

    pub fn children(
        &self,
        id: CandidateId,
    ) -> Vec<CandidateId>;

    pub fn lineage(
        &self,
        id: CandidateId,
    ) -> Lineage<'g, P>;

    pub fn siblings(
        &self,
        id: CandidateId,
    ) -> Vec<CandidateId>;

    pub fn informed_by(
        &self,
        id: CandidateId,
    ) -> Vec<InfoRef>;

    pub fn informed(
        &self,
        id: CandidateId,
    ) -> Vec<CandidateId>;

    pub fn proposal_batch(
        &self,
        id: ProposalBatchId,
    ) -> Option<ProposalBatchView<'g, P>>;

    pub fn proposal_that_created(
        &self,
        id: CandidateId,
    ) -> Option<ProposalView<'g, P>>;

    pub fn assessments(
        &self,
        id: CandidateId,
    ) -> AssessmentQuery<'g, P>;

    pub fn pairwise_assessments(
        &self,
        left: CandidateId,
        right: CandidateId,
    ) -> AssessmentQuery<'g, P>;

    pub fn candidate_tree(
        &self,
    ) -> CandidateTree<'g, P>;

    pub fn costs(
        &self,
    ) -> CostSummary;
}
```

## View invariants

```text
RunGraphView never mutates.
RunGraphView respects ReadScope.
lineage follows causal inputs only.
informed_by follows informational provenance only.
assessment queries hide forbidden evidence/targets according to ReadScope.
```

---

# 15. RunGraph Data Model Tests

## 15.1 Unit tests

### `create_proposal_creates_candidate_without_causal_parent`

Setup:

```text
seed graph empty
proposal effect = Create { artifact: A }
provenance.causal = None
```

Assert:

```text
apply succeeds
candidate exists
graph.parents(candidate) == []
proposal_that_created(candidate) == proposal_id
candidate.origin = Proposal
```

### `change_proposal_requires_target_in_causal_inputs`

Setup:

```text
seed candidate A
proposal effect = Change { target: A, change }
provenance.causal = None
```

Assert:

```text
apply fails
ApplyFailed recorded
no candidate created
error kind = GraphInvariant or Apply
```

### `change_proposal_creates_causal_edge`

Setup:

```text
seed A
proposal effect = Change { target: A, change }
provenance.causal = Single(A)
```

Assert:

```text
apply succeeds -> B
graph.parents(B) == [A]
graph.children(A) contains B
```

### `merge_proposal_records_pair_lineage_but_applies_to_one_target`

Setup:

```text
seed A, B
proposal effect = Change { target: A, change_importing_content_from_B }
provenance.causal = Pair(A, B)
```

Assert:

```text
apply calls A.apply_change(change)
graph.parents(child) == [A, B]
graph.children(A) and graph.children(B) include child
```

### `informed_by_does_not_affect_lineage`

Setup:

```text
seed A, B
proposal effect = Change { target: A, change }
causal = Single(A)
informed_by = [Candidate(B)]
```

Assert:

```text
parents(child) == [A]
informed_by(child) == [Candidate(B)]
children(B) does not include child
informed(B) includes child
```

### `same_content_can_have_multiple_candidates`

Setup:

```text
seed A
two Create proposals with artifact identity X
```

Assert:

```text
candidate ids differ
identity equal
by_identity[X] returns both candidates
lineage separate
```

### `failed_apply_records_attempt`

Setup:

```text
seed A
proposal effect = Change with invalid change
```

Assert:

```text
apply attempt exists
outcome = Failed
proposal has apply attempt
no candidate created
ApplyFailed event emitted
```

## 15.2 Property tests

Use `proptest`.

### Graph append-only

Generate random valid operations:

```text
insert seed
record proposal
apply proposal
record assessment
record population event
```

Assert:

```text
record counts never decrease
existing records never mutate
event ordering monotonic
```

### Causal lineage is acyclic for Change proposals

If graph forbids causal cycles:

```text
for every candidate, lineage traversal terminates
candidate is not its own ancestor
```

For `Create` with causal inputs, also ensure referenced candidates pre-exist.

### Informed graph may be cyclic

Information flow may be cyclic across iterations? In practice, a proposal can only be informed by existing records, so it is temporally acyclic. Assert:

```text
InfoRef targets exist before proposal timestamp
```

### Proposal application is idempotently rejected

Applying the same proposal twice:

```text
first apply succeeds or fails
second apply returns ProposalAlreadyApplied
no second candidate
```

## 15.3 Golden event tests

For a simple change:

```text
ProposalBatchProduced
ProposalRecorded
ApplySucceeded
```

For failed apply:

```text
ProposalBatchProduced
ProposalRecorded
ApplyFailed
Error
```

The event stream is part of the public debugging story.

---

# 16. Subsystem: RunContext Services

## 16.1 Module layout

```text
optimize-core/src/context/
  mod.rs
  run_context.rs
  proposal_context.rs
  evaluation_context.rs
  render_context.rs
  trust.rs

optimize-core/src/stage/
  evaluator.rs
  proposer.rs
  callback.rs
  stopper.rs

optimize-core/src/engine.rs
```

---

# 17. Engine Shape

```rust
// engine.rs

pub struct Engine<P, O>
where
    P: OptimizationProblem,
    O: Optimizer<P>,
{
    problem: P,
    optimizer: O,

    graph: RunGraph<P>,
    case_set: Option<CaseSet<P::Case>>,

    evaluators: EvaluatorRegistry<P>,
    evidence_store: Box<dyn EvidenceStore<P::Evidence>>,
    cache: EvaluationCache<P>,

    budget: BudgetLedger,
    callbacks: Vec<Box<dyn DynCallback<P>>>,
    stoppers: Vec<Box<dyn DynStopper<P>>>,

    trust: TrustPolicy,
    rng: rand::rngs::StdRng,
    store: Box<dyn RunStore<P>>,
}
```

The engine owns state. `RunContext` borrows it mutably during optimizer calls.

---

# 18. RunContext

```rust
// context/run_context.rs

pub struct RunContext<'e, P: OptimizationProblem> {
    graph: &'e mut RunGraph<P>,
    case_set: Option<&'e CaseSet<P::Case>>,

    evaluators: &'e EvaluatorRegistry<P>,
    evidence_store: &'e dyn EvidenceStore<P::Evidence>,
    cache: &'e mut EvaluationCache<P>,

    budget: &'e mut BudgetLedger,
    callbacks: &'e mut [Box<dyn DynCallback<P>>],
    trust: &'e TrustPolicy,

    iteration: Option<IterationId>,
    actor: Actor,
    read_scope: ReadScope,

    rng: &'e mut rand::rngs::StdRng,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Actor {
    Optimizer,
    Proposer(ProposerId),
    Evaluator(EvaluatorId),
    Renderer(StageId),
    Callback,
}
```

## Context invariant

```text
RunContext is the only public mutation path into RunGraph.
All context methods enforce budget, trust, events, cache, evidence-store, and error normalization.
No optimizer receives &mut RunGraph directly.
```

---

# 19. RunContext API

```rust
impl<'e, P: OptimizationProblem> RunContext<'e, P> {
    pub fn graph(&self) -> RunGraphView<'_, P> {
        self.graph.view(self.read_scope.clone())
    }

    pub fn iteration(&self) -> Option<IterationId> {
        self.iteration
    }

    pub fn budget(&self) -> BudgetSnapshot {
        self.budget.snapshot()
    }

    pub fn rng(&mut self) -> &mut rand::rngs::StdRng {
        self.rng
    }

    pub async fn propose<Pr>(
        &mut self,
        proposer: &Pr,
        request: Pr::Request,
    ) -> Result<ProposalBatchReport, ProposalError>
    where
        Pr: Proposer<P>,
    {
        // builds ProposalContext
        // calls proposer
        // charges budget
        // records batch + proposals
        // emits events
    }

    pub fn record_proposal_batch(
        &mut self,
        stage: StageId,
        batch: ProposalBatch<P>,
        cost: Cost,
    ) -> Result<ProposalBatchReport, ContextError> {
        // for optimizers that create proposals directly without a Proposer stage
    }

    pub async fn apply_batch(
        &mut self,
        batch_id: ProposalBatchId,
    ) -> Result<ApplyReport, ApplyContextError> {
        // applies all proposals in recorded batch
    }

    pub async fn apply_proposal(
        &mut self,
        proposal_id: ProposalId,
    ) -> Result<ApplyOneReport, ApplyContextError> {
        // apply one proposal record
    }

    pub async fn evaluate(
        &mut self,
        request: EvaluationRequest,
    ) -> Result<EvaluationReport, EvaluationContextError> {
        self.evaluate_with(EvaluatorId::PRIMARY, request).await
    }

    pub async fn evaluate_with(
        &mut self,
        evaluator_id: EvaluatorId,
        request: EvaluationRequest,
    ) -> Result<EvaluationReport, EvaluationContextError> {
        // trust check
        // resolve EvaluationSet
        // cache lookup
        // call evaluator
        // charge budget
        // store evidence
        // record assessments
        // emit events
    }

    pub fn compare(
        &self,
        left: CandidateId,
        right: CandidateId,
        scope: PreferenceScope,
        relation: &dyn DynPreferenceRelation<P>,
    ) -> Preference {
        relation.compare(left, right, scope, self.graph())
    }

    pub fn record_population_events(
        &mut self,
        population: PopulationId,
        events: Vec<PopulationEvent>,
    ) {
        self.graph.record_population_events(population, events.clone());
        self.emit(RunEvent::PopulationUpdated {
            population_id: population,
            events,
        });
    }

    pub fn charge(
        &mut self,
        stage: StageId,
        cost: Cost,
    ) -> Result<(), BudgetExceeded> {
        let remaining = self.budget.charge(stage, cost.clone())?;
        self.graph.record_budget_event(BudgetEventRecord {
            stage,
            cost: cost.clone(),
            remaining: remaining.clone(),
            when: chrono::Utc::now(),
        });
        self.emit(RunEvent::BudgetCharged {
            stage,
            cost,
            remaining,
        });
        Ok(())
    }

    pub fn emit(&mut self, event: RunEvent<P>) {
        self.graph.record_event(event.clone());
        for cb in self.callbacks.iter_mut() {
            cb.on_event_boxed(&event, self.graph.view(self.read_scope.clone()));
        }
    }
}
```

`RunContext::propose`, `evaluate`, and render methods are where costful user code enters. They should all route through `charge`.

---

# 20. Proposer Trait

```rust
// stage/proposer.rs

pub trait Proposer<P: OptimizationProblem>: Send + Sync {
    type Request: Send + Sync + 'static;

    fn id(&self) -> ProposerId;

    fn arity(&self) -> Arity;

    async fn propose(
        &self,
        request: Self::Request,
        ctx: ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arity {
    None,
    Single,
    Pair,
    Variadic,
}
```

`Arity` is a request hint for optimizers that perform parent selection. It is not a law on every proposal in the batch.

---

# 21. RunContext::propose Semantics

```rust
pub async fn propose<Pr>(
    &mut self,
    proposer: &Pr,
    request: Pr::Request,
) -> Result<ProposalBatchReport, ProposalError>
where
    Pr: Proposer<P>,
{
    let stage = StageId::from_proposer(proposer.id());

    let proposal_ctx = ProposalContext {
        graph: self.graph.view(self.trust.proposer_read_scope(proposer.id())),
        budget: self.budget.proposer_budget_handle(stage),
        read_scope: self.trust.proposer_read_scope(proposer.id()),
        eval: self.trust.eval_handle_for_proposer(proposer.id()),
        workspace: self.trust.workspace_factory_for_proposer(proposer.id()),
        // renderers, evidence store, etc.
    };

    let metered = proposer.propose(request, proposal_ctx).await.map_err(|err| {
        self.record_stage_error(stage, err.as_record());
        err
    })?;

    self.charge(stage, metered.cost.clone())?;

    let report = self.record_proposal_batch(
        stage,
        metered.value,
        metered.cost,
    )?;

    self.emit(RunEvent::ProposalBatchProduced {
        iteration: self.iteration,
        batch_id: report.batch_id,
        proposer: stage,
        proposal_count: report.proposal_ids.len(),
    });

    Ok(report)
}
```

## Propose invariants

```text
No proposal batch enters graph without a stage id.
Proposer cost is charged before the report is returned.
Proposer errors are recorded as Error events.
ProposalContext graph view is read-scoped.
```

---

# 22. Apply Reports

```rust
pub struct ProposalBatchReport {
    pub batch_id: ProposalBatchId,
    pub proposal_ids: Vec<ProposalId>,
    pub cost: Cost,
}

pub struct ApplyReport {
    pub batch_id: ProposalBatchId,
    pub outcomes: Vec<ApplyOneReport>,
}

pub struct ApplyOneReport {
    pub proposal_id: ProposalId,
    pub attempt_id: ApplyAttemptId,
    pub outcome: ApplyOneOutcome,
}

pub enum ApplyOneOutcome {
    Created {
        candidate_id: CandidateId,
        identity: ArtifactIdentity,
    },

    Failed {
        error: ErrorRecord,
    },
}
```

## RunContext::apply_batch

```rust
pub async fn apply_batch(
    &mut self,
    batch_id: ProposalBatchId,
) -> Result<ApplyReport, ApplyContextError> {
    let proposal_ids = self
        .graph
        .proposal_batch(batch_id)
        .ok_or(ApplyContextError::UnknownBatch(batch_id))?
        .proposal_ids
        .clone();

    let mut outcomes = Vec::with_capacity(proposal_ids.len());

    for proposal_id in proposal_ids {
        let one = self.apply_proposal(proposal_id).await?;
        outcomes.push(one);
    }

    Ok(ApplyReport { batch_id, outcomes })
}
```

## RunContext::apply_proposal

```rust
pub async fn apply_proposal(
    &mut self,
    proposal_id: ProposalId,
) -> Result<ApplyOneReport, ApplyContextError> {
    let attempt = self.graph.apply_proposal_record(proposal_id);

    let outcome = match &attempt.outcome {
        ApplyOutcome::Created { candidate_id, identity } => {
            self.emit(RunEvent::ApplySucceeded {
                proposal_id,
                candidate_id: *candidate_id,
                identity: identity.clone(),
            });

            ApplyOneOutcome::Created {
                candidate_id: *candidate_id,
                identity: identity.clone(),
            }
        }

        ApplyOutcome::Failed { error } => {
            self.emit(RunEvent::ApplyFailed {
                proposal_id,
                error: error.clone(),
            });

            ApplyOneOutcome::Failed {
                error: error.clone(),
            }
        }
    };

    Ok(ApplyOneReport {
        proposal_id,
        attempt_id: attempt.id,
        outcome,
    })
}
```

Apply does not charge by default. If artifact validation becomes costful, it should be modeled as an evaluator or a metered validation stage, not hidden in `Artifact::apply_change`.

---

# 23. Evaluator Trait

```rust
// stage/evaluator.rs

pub trait Evaluator<P: OptimizationProblem>: Send + Sync {
    fn id(&self) -> EvaluatorId;

    fn fingerprint(&self) -> Fingerprint;

    fn cache_policy(&self, request: &EvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'_, P>,
    ) -> Result<Metered<Vec<Assessment<P>>>, EvaluationError>;
}
```

Important: `Evaluator` receives `ResolvedEvaluationRequest`, not raw `EvaluationRequest`, so dynamic sets are frozen before evaluation.

---

# 24. RunContext::evaluate_with Semantics

```rust
pub async fn evaluate_with(
    &mut self,
    evaluator_id: EvaluatorId,
    request: EvaluationRequest,
) -> Result<EvaluationReport, EvaluationContextError> {
    let evaluator = self
        .evaluators
        .get(evaluator_id)
        .ok_or(EvaluationContextError::UnknownEvaluator(evaluator_id))?;

    self.trust
        .check_evaluation_request(self.actor, &request)
        .map_err(EvaluationContextError::Trust)?;

    let resolved = self
        .resolve_evaluation_request(request)
        .map_err(EvaluationContextError::Resolve)?;

    let request_id = self.graph.record_evaluation_request(
        evaluator_id,
        resolved.clone(),
    );

    self.emit(RunEvent::EvaluationRequested {
        request_id,
        evaluator: evaluator_id,
        request: resolved.summary(),
    });

    if let Some(hit) = self.try_eval_cache(evaluator, &resolved)? {
        self.emit(RunEvent::EvaluationCompleted {
            request_id,
            evaluator: evaluator_id,
            assessment_ids: hit.assessment_ids.clone(),
            cost: Cost::zero(),
            cache: CacheStatus::Hit,
        });
        return Ok(hit);
    }

    let eval_ctx = EvaluationContext {
        graph: self.graph.view(self.trust.evaluator_read_scope(evaluator_id)),
        budget: self.budget.evaluator_budget_handle(evaluator_id),
        evidence_store: self.evidence_store,
        read_scope: self.trust.evaluator_read_scope(evaluator_id),
        // workspace if evaluator has one
    };

    let metered = evaluator
        .evaluate(resolved.clone(), eval_ctx)
        .await
        .map_err(|err| {
            self.record_stage_error(StageId::from_evaluator(evaluator_id), err.as_record());
            EvaluationContextError::Evaluator(err)
        })?;

    self.charge(StageId::from_evaluator(evaluator_id), metered.cost.clone())?;

    let stored = self.store_assessment_evidence(metered.value)?;
    let assessment_ids = self.graph.record_assessments(
        request_id,
        evaluator_id,
        stored,
    );

    self.insert_eval_cache_if_allowed(evaluator, &resolved, &assessment_ids)?;

    self.emit(RunEvent::EvaluationCompleted {
        request_id,
        evaluator: evaluator_id,
        assessment_ids: assessment_ids.clone(),
        cost: metered.cost,
        cache: CacheStatus::Miss,
    });

    Ok(EvaluationReport {
        request_id,
        assessment_ids,
    })
}
```

## Evaluate invariants

```text
EvaluationSet is resolved before evaluator runs.
TrustPolicy is checked before evaluator runs.
Cache is checked before evaluator runs.
Evaluator cost is charged after evaluator returns, even if some cases failed.
Evidence is stored before assessment records are inserted.
Assessment records refer to EvidenceRef, not inline evidence.
EvaluationCompleted is emitted exactly once per request.
```

If the evaluator fails after spending cost, it should return an error type that carries `Cost`:

```rust
pub struct EvaluationError {
    pub record: ErrorRecord,
    pub spent: Cost,
}
```

Then context charges `spent` before returning error.

---

# 25. Evidence Store

```rust
// evidence.rs

pub trait Evidence: Send + Sync + 'static {}

pub trait EvidenceStore<E: Evidence>: Send + Sync {
    fn put(&self, evidence: E) -> Result<EvidenceRef, StoreError>;

    fn get(&self, reference: &EvidenceRef) -> Result<E, StoreError>;
}
```

`RunContext::store_assessment_evidence` converts:

```rust
Assessment<P>
```

to:

```rust
StoredAssessment
```

by storing `P::Evidence` and replacing it with `EvidenceRef`.

---

# 26. Trust

```rust
// context/trust.rs

#[derive(Clone, Debug)]
pub struct TrustPolicy {
    hidden_from_proposers: Vec<PartitionId>,
    hidden_from_optimizers: Vec<PartitionId>,
    hidden_from_callbacks: Vec<PartitionId>,
    allowed_probe_sets: ProbePolicy,
}

#[derive(Clone, Debug)]
pub struct ReadScope {
    visible_partitions: std::collections::BTreeSet<PartitionId>,
    visible_evidence: EvidenceVisibility,
}

#[derive(Clone, Debug)]
pub enum EvidenceVisibility {
    Full,
    ScoresOnly,
    SummariesOnly,
    None,
}

impl TrustPolicy {
    pub fn proposer_read_scope(&self, proposer: ProposerId) -> ReadScope;

    pub fn evaluator_read_scope(&self, evaluator: EvaluatorId) -> ReadScope;

    pub fn optimizer_read_scope(&self) -> ReadScope;

    pub fn check_evaluation_request(
        &self,
        actor: Actor,
        request: &EvaluationRequest,
    ) -> Result<(), TrustViolation>;
}
```

## Trust invariants

```text
Graph views are read-scoped.
Renderers receive read scope.
Evidence queries respect read scope.
Evaluation requests are checked against actor permissions.
Forbidden partition evidence cannot be rendered into proposer workspaces.
Trust violations are Error events.
```

The framework cannot stop a user-written optimizer from embedding hidden data into a custom proposer request if the optimizer itself has access. The framework can make the right boundary easy and violations visible.

---

# 27. Cache

```rust
pub enum CachePolicy {
    Never,
    Deterministic,
    DeterministicWithSeed(u64),
    UserKey(Fingerprint),
}

pub struct EvaluationCacheKey {
    evaluator_fingerprint: Fingerprint,
    request_fingerprint: Fingerprint,
    candidate_identities: Vec<ArtifactIdentity>,
    evaluation_set_id: EvaluationSetId,
    case_set_version: CaseSetVersion,
    seed: Option<u64>,
}
```

Cache invariants:

```text
default is Never.
Deterministic cache requires content identities, not merely external identities, unless UserKey is supplied.
evaluator fingerprint is part of key.
resolved evaluation set id is part of key.
case set version is part of key.
pairwise order is preserved unless evaluator declares symmetry.
```

---

# 28. Callback Events

Events must be aligned with v0.2.1 proposal model.

```rust
pub enum RunEvent<P: OptimizationProblem> {
    OptimizationStarted {
        run_id: RunId,
    },

    OptimizationStopping {
        reason: StopReason,
    },

    OptimizationEnded {
        run_id: RunId,
        best: Option<CandidateId>,
        budget: BudgetSnapshot,
    },

    IterationStarted {
        iteration: IterationId,
    },

    IterationEnded {
        iteration: IterationId,
        status: StepStatus,
    },

    ProposalBatchProduced {
        iteration: Option<IterationId>,
        batch_id: ProposalBatchId,
        proposer: StageId,
        proposal_count: usize,
    },

    ProposalRecorded {
        proposal_id: ProposalId,
        batch_id: ProposalBatchId,
        effect: ProposalEffectSummary,
        causal: CausalInputs,
        informed_by_count: usize,
    },

    ApplySucceeded {
        proposal_id: ProposalId,
        candidate_id: CandidateId,
        identity: ArtifactIdentity,
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

    PopulationUpdated {
        population_id: PopulationId,
        events: Vec<PopulationEvent>,
    },

    BudgetCharged {
        stage: StageId,
        cost: Cost,
        remaining: BudgetSnapshot,
    },

    Error {
        stage: Option<StageId>,
        error: ErrorRecord,
        policy: ErrorPolicy,
    },
}
```

No batch-level `parent_ids`. Parents are proposal-level provenance.

---

# 29. RunContext Tests

## 29.1 Propose records and charges

Test:

```text
dummy proposer returns Metered<ProposalBatch> cost = 3 LLM calls
ctx.propose(dummy, request)
```

Assert:

```text
ProposalBatchRecord exists
ProposalRecord exists for each proposal
BudgetCharged event exists
ProposalBatchProduced event exists
ProposalRecorded events exist
remaining budget decreased
```

## 29.2 Proposer error records stage error

Test:

```text
dummy proposer returns ProposalError with spent cost
```

Assert:

```text
Error event emitted
spent cost charged if error carries cost
no ProposalBatchRecord inserted
```

## 29.3 Apply batch creates candidates

Test:

```text
record batch with two Create proposals
ctx.apply_batch(batch_id)
```

Assert:

```text
two candidates created
two ApplySucceeded events
candidate origins point to proposal/apply attempt
```

## 29.4 Apply batch partial failure

Test:

```text
batch has one valid Change and one invalid Change
```

Assert:

```text
one candidate created
one ApplyFailed event
ApplyReport has both outcomes
batch application does not abort on one failure
```

## 29.5 Evaluate uses cache when deterministic

Setup:

```text
artifact implements ContentAddressed
evaluator cache_policy = Deterministic
same request twice
```

Assert:

```text
first call invokes evaluator
second call does not invoke evaluator
second call emits EvaluationCompleted cache=Hit
```

## 29.6 Evaluate does not cache by default

Setup:

```text
evaluator cache_policy = Never
same request twice
```

Assert:

```text
evaluator invoked twice
two EvaluationRequestRecords
two AssessmentRecords
```

## 29.7 Evaluate stores evidence externally

Setup:

```text
evaluator returns Assessment with large evidence
```

Assert:

```text
EvidenceStore::put called
AssessmentRecord stores EvidenceRef
graph does not inline evidence
graph.view().assessment(id).evidence() loads from store
```

## 29.8 Trust hides forbidden partition from proposer

Setup:

```text
TrustPolicy hides TEST from proposer
graph has TEST assessment
proposer context renders history
```

Assert:

```text
proposer RunGraphView does not expose TEST assessment evidence
renderer does not write TEST evidence into workspace
TrustViolation if proposer eval handle requests TEST
```

## 29.9 Budget exhaustion stops context method

Setup:

```text
budget allows 1 LLM call
proposer cost = 2 LLM calls
```

Assert:

```text
ctx.propose returns BudgetExceeded
BudgetCharged either absent or records attempted/partial according to policy
Error event emitted
engine stops with StopReason::BudgetExceeded
```

## 29.10 Callbacks receive event order

Test small run:

```text
OptimizationStarted
IterationStarted
ProposalBatchProduced
ProposalRecorded
ApplySucceeded
EvaluationRequested
EvaluationCompleted
PopulationUpdated
BudgetCharged
IterationEnded
OptimizationEnded
```

Assert callback saw monotonic event order and graph view was readable.

---

# 30. Compile-Fail Tests

Use `trybuild`.

## 30.1 Cannot use part selector without an edit surface

```rust
Gepa::default()
    .part_selector(RoundRobinPart)
```

with no compatible `EditSurface` should fail at the GEPA builder bound.

## 30.2 Cannot use deterministic cache with non-content-addressed artifact unless UserKey

If you choose to enforce at type level:

```rust
DeterministicCache<Evaluator, A: ContentAddressed>
```

Should fail for external-only artifacts.

If runtime-enforced, test returns config error:

```text
CachePolicy::Deterministic rejected for ArtifactIdentity::External
```

## 30.3 Cannot use pairwise tournament population with non-pairwise evidence

```rust
TournamentPopulation<PairwiseEvidence>
```

should require:

```rust
P::Evidence: PairwiseEvidence
```

## 30.4 Cannot use typed claim gate without matching annotations

```rust
ClaimsHeldAcceptance<EditAnnotations>
```

should require:

```rust
P::ProposalAnnotations: HasBehavioralClaims
```

---

# 31. Property Tests

## 31.1 Append-only graph

Random sequence of valid operations:

```text
insert seed
record proposal batch
apply proposal
record assessment
record population event
```

Assert previous records remain byte-equal.

## 31.2 Causal lineage references only older candidates

For every candidate created by proposal:

```text
all CausalInputs existed before proposal was applied
```

## 31.3 Informed refs reference existing records

Every `InfoRef` must reference either:

```text
existing candidate/proposal/assessment
or ExternalRef
```

No dangling internal references.

## 31.4 Same identity, multiple candidates

Generate proposals that create identical artifacts.

Assert:

```text
by_identity maps to all candidates
candidate ids are distinct
lineage remains distinct
```

## 31.5 Apply idempotence

Applying a proposal twice:

```text
first outcome recorded
second attempt rejected as already applied
no new candidate
```

---

# 32. Golden Integration Tests

## 32.1 P0 graph skeleton

```text
Artifact = TestStringArtifact
ProposalEffect::Create
ProposalEffect::Change
No evaluator
```

Asserts graph/proposal/apply invariants.

## 32.2 P1 scalar keep-best

```text
EvaluatorFn returns ScalarEvidence
Population = KeepBest
Optimizer proposes Change alternatives
```

Asserts engine/context/population/event flow.

## 32.3 P2 pairwise tournament

```text
EvaluationRequest::Pairwise
PairwiseJudgmentEvidence
TournamentPopulation owns fitted model
```

Asserts pairwise assessments and population updates.

## 32.4 P3 GEPA parity

```text
PartMapArtifact
ProposalBatch::Alternatives
PerCase assessments
ParetoFrontier::by_case
```

Asserts instance-wise Pareto behavior.

## 32.5 P4 workspace / Meta-Harness-lite

```text
ProposalEffect::Create
ProposalProvenance.informed_by
EvidenceStore refs
TrustPolicy hides TEST
Workspace renderer writes visible history only
```

Asserts agentic boundaries without needing real Claude Code.

---

# 33. Implementation Order for These Two Subsystems

I would implement in this order:

```text
1. ids.rs, metadata.rs, error.rs
2. artifact.rs, problem.rs
3. proposal.rs with ProposalEffect + ProposalProvenance
4. candidate.rs
5. graph/storage.rs with only seeds/proposals/apply
6. graph/view.rs with parents/children/lineage/informed_by
7. RunContext::record_proposal_batch + apply_batch
8. event emission for proposal/apply
9. evaluation.rs + assessment records
10. EvidenceStore trait + InlineEvidenceStore test impl
11. RunContext::evaluate_with without cache
12. BudgetLedger + BudgetCharged
13. EvaluationCache
14. TrustPolicy + read-scoped RunGraphView
15. callback event tests
```

Do not implement GEPA until steps 1–14 are stable.

---

# 34. Final Shape Summary

The two core subsystems should make these statements true:

```text
A proposal either creates an artifact or changes exactly one target candidate.
Causal lineage and informational provenance are different typed facts.
A proposal batch groups stage siblings; it does not define parentage.
A candidate is a graph-local occurrence, not merely content.
Same content can appear in multiple candidates.
RunGraph is append-only truth.
Population events are strategy opinions.
RunContext is the only mutation surface.
Every costly context method charges budget.
Every fallible boundary produces typed error records.
Every major operation emits events.
Trust is enforced by scoped views and context handles.
Evidence is stored by ref; graph records remain lightweight.
```

That is the exact substrate the rest of the optimizer library depends on.
