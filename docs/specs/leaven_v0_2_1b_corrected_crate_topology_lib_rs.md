# Leaven v0.2.3 - Corrected Crate Topology and `lib.rs` Maps

*Topology/interface spec after fixing cold-core projection leaks, surface ownership, workspace materialization, and GEPA selector seams.*

> Status: topology correction pass, pre-implementation.  
> Target spec: Leaven v0.2.3.  
> This supersedes the v0.2.1b crate-topology draft.  
> Purpose: make crate boundaries enforce the real knowledge graph before P0/P1 implementation.

---

## 0. Executive Correction Summary

The v0.2.1a crate graph was close, but it still violated the requirements in a few places by making domain projections look intrinsic.

The largest bug:

```text
Artifact -> Decomposable -> Component
```

That is not artifact-shape neutral. It works for structs and skill banks, but not for git/jj repos, arbitrary codebases, patch stacks, worktrees, or changeset graphs.

The correction:

```text
Artifact = intrinsic thing being optimized
EditSurface = chosen projection/lens over an artifact
Renderer = chosen way to show/serialize a value for LM calls, debug, or typed views
Materializer = chosen way to materialize a value into a workspace for agents/sandboxes
```

So:

```text
repos do not have components
layouts/surfaces over repos expose parts
paths are addresses, not necessarily identities
renames preserve identity only if the surface explicitly encodes logical IDs
```

Other corrections:

1. **`Decomposable` leaves `leaven-core`.**  
   It is replaced by `leaven-surface::EditSurface`.

2. **Evidence separates casewise measurement from attribution.**  
   No `ComponentEvidence` in core. Use `CasewiseEvidence` for per-case outcomes and `AttributableEvidence<K>` for blame/credit/routing where `K` may be a surface part ID, path, agent ID, changeset ID, module ID, etc.

3. **`Materializable` is not cold core and not in artifacts.**  
   Workspace materialization is an agentic/sandbox layout concern. Standard bridge helpers live outside artifacts/core.

4. **`leaven-store` does not own `RunStore<P>`.**  
   It owns generic blob/evidence/checkpoint-byte stores. Engine owns graph persistence codec.

5. **`RunGraph` storage and `Engine` remain together.**  
   Only `RunContext` may mutate graph. This requires graph storage and engine to share a crate.

6. **Reports are ID-only.**  
   No borrowed graph views returned from `&mut RunContext` methods.

7. **Proposer requests are associated types.**  
   No universal `ProposalRequest<P>` god enum.

8. **Renderer registries are secondary.**  
   Stage-owned renderers are the default path.

This topology intentionally prioritizes correctness over “few crates.” The goal is not minimal crate count; it is compiler-enforced knowledge boundaries.

---

## 1. Workspace Layout

```text
leaven/
├── Cargo.toml
├── crates/
│   ├── leaven-kernel/
│   ├── leaven-core/
│   ├── leaven-surface/
│   │
│   ├── leaven-store/
│   ├── leaven-store-inline/
│   ├── leaven-store-file/
│   ├── leaven-store-sqlite/
│   ├── leaven-store-object/
│   │
│   ├── leaven-workspace/
│   ├── leaven-workspace-local/
│   ├── leaven-workspace-docker/
│   ├── leaven-workspace-e2b/
│   ├── leaven-workspace-k8s/
│   ├── leaven-workspace-firecracker/
│   ├── leaven-workspace-git/
│   │
│   ├── leaven-engine/
│   ├── leaven-derive/
│   │
│   ├── leaven-artifacts/
│   ├── leaven-artifact-git/
│   ├── leaven-artifact-jj/
│   ├── leaven-evidence/
│   ├── leaven-preference/
│   ├── leaven-population/
│   ├── leaven-render/
│   ├── leaven-std/
│   │
│   ├── leaven-lm/
│   ├── leaven-lm-openai/
│   ├── leaven-lm-anthropic/
│   ├── leaven-lm-local/
│   ├── leaven-lm-mock/
│   │
│   ├── leaven-agent/
│   ├── leaven-agent-claude-code/
│   ├── leaven-agent-codex/
│   ├── leaven-agent-opencode/
│   ├── leaven-agentic/
│   ├── leaven-agentic-skill/
│   │
│   ├── leaven-gepa/
│   ├── leaven-mipro/
│   ├── leaven-textgrad/
│   ├── leaven-trace/
│   │
│   ├── leaven-dsrs/
│   ├── leaven-cuda/
│   ├── leaven-python/
│   │
│   └── leaven/
│
├── examples/
│   ├── p0_graph_skeleton/
│   ├── p1_keep_best/
│   ├── p2_pairwise_tournament/
│   ├── p3_gepa_parity/
│   └── p4_meta_harness_lite/
│
├── xtask/
└── scripts/
    └── check_crate_dag.rs
```

---

## 2. Root `Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
    "crates/leaven-kernel",
    "crates/leaven-core",
    "crates/leaven-surface",

    "crates/leaven-store",
    "crates/leaven-store-inline",
    "crates/leaven-store-file",
    "crates/leaven-store-sqlite",
    "crates/leaven-store-object",

    "crates/leaven-workspace",
    "crates/leaven-workspace-local",
    "crates/leaven-workspace-docker",
    "crates/leaven-workspace-e2b",
    "crates/leaven-workspace-k8s",
    "crates/leaven-workspace-firecracker",
    "crates/leaven-workspace-git",

    "crates/leaven-engine",
    "crates/leaven-derive",

    "crates/leaven-artifacts",
    "crates/leaven-artifact-git",
    "crates/leaven-artifact-jj",
    "crates/leaven-evidence",
    "crates/leaven-preference",
    "crates/leaven-population",
    "crates/leaven-render",
    "crates/leaven-std",

    "crates/leaven-lm",
    "crates/leaven-lm-openai",
    "crates/leaven-lm-anthropic",
    "crates/leaven-lm-local",
    "crates/leaven-lm-mock",

    "crates/leaven-agent",
    "crates/leaven-agent-claude-code",
    "crates/leaven-agent-codex",
    "crates/leaven-agent-opencode",
    "crates/leaven-agentic",
    "crates/leaven-agentic-skill",

    "crates/leaven-gepa",
    "crates/leaven-mipro",
    "crates/leaven-textgrad",
    "crates/leaven-trace",

    "crates/leaven-dsrs",
    "crates/leaven-cuda",
    "crates/leaven-python",

    "crates/leaven",

    "examples/p0_graph_skeleton",
    "examples/p1_keep_best",
    "examples/p2_pairwise_tournament",
    "examples/p3_gepa_parity",
    "examples/p4_meta_harness_lite",

    "xtask",
]

[workspace.package]
edition = "2024"
license = "MIT OR Apache-2.0"
repository = "https://github.com/<org>/leaven"
rust-version = "1.85"

[workspace.dependencies]
anyhow = "1"
async-trait = "0.1"
blake3 = "1"
bytes = "1"
chrono = { version = "0.4", features = ["serde"] }
futures = "0.3"
indexmap = { version = "2", features = ["serde"] }
ordered-float = { version = "4", features = ["serde"] }
parking_lot = "0.12"
rand = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
smallvec = { version = "1", features = ["serde"] }
smol_str = { version = "0.2", features = ["serde"] }
thiserror = "1"
tokio = { version = "1", features = ["rt", "macros", "process", "fs", "time", "sync"] }
uuid = { version = "1", features = ["serde", "v4"] }

proc-macro2 = "1"
quote = "1"
syn = "2"
```

---

## 3. Dependency Allowlist

This should be CI-enforced.

### Foundation

```text
leaven-kernel -> []

leaven-core -> [
  leaven-kernel
]

leaven-surface -> [
  leaven-kernel,
  leaven-core
]

leaven-store -> [
  leaven-kernel,
  leaven-core
]

leaven-workspace -> [
  leaven-kernel
]
```

### Engine

```text
leaven-engine -> [
  leaven-kernel,
  leaven-core,
  leaven-store,
  leaven-workspace
]
```

Engine is artifact-shape-neutral. Surface-aware query helpers belong in
`leaven-gepa`, `leaven-render`, or optimizer/stage helper crates, not in engine.

### Derive

```text
leaven-derive -> [
  leaven-kernel,
  leaven-core,
  leaven-surface
]
```

Derive may generate both artifact impls and surfaces.

### Standard crates

```text
leaven-artifacts -> [
  leaven-kernel,
  leaven-core,
  leaven-surface
]

leaven-artifact-git -> [
  leaven-kernel,
  leaven-core,
  leaven-surface,
  leaven-artifacts
]

leaven-artifact-jj -> [
  leaven-kernel,
  leaven-core,
  leaven-surface,
  leaven-artifacts
]

leaven-evidence -> [
  leaven-kernel,
  leaven-core
]

leaven-preference -> [
  leaven-kernel,
  leaven-core,
  leaven-engine,
  leaven-evidence
]

leaven-population -> [
  leaven-kernel,
  leaven-core,
  leaven-engine,
  leaven-evidence,
  leaven-preference
]

leaven-render -> [
  leaven-kernel,
  leaven-core,
  leaven-surface,
  leaven-store,
  leaven-workspace,
  leaven-engine,
  leaven-artifacts,
  leaven-evidence
]

leaven-std -> [
  leaven-artifacts,
  leaven-evidence,
  leaven-preference,
  leaven-population,
  leaven-render
]
```

### LLM and agent

```text
leaven-lm -> [
  leaven-kernel
]

leaven-lm-openai -> [
  leaven-kernel,
  leaven-lm
]

leaven-lm-anthropic -> [
  leaven-kernel,
  leaven-lm
]

leaven-lm-local -> [
  leaven-kernel,
  leaven-lm
]

leaven-lm-mock -> [
  leaven-kernel,
  leaven-lm
]

leaven-agent -> [
  leaven-kernel,
  leaven-workspace
]

leaven-agent-claude-code -> [
  leaven-kernel,
  leaven-workspace,
  leaven-agent
]

leaven-agent-codex -> [
  leaven-kernel,
  leaven-workspace,
  leaven-agent
]

leaven-agent-opencode -> [
  leaven-kernel,
  leaven-workspace,
  leaven-agent
]

leaven-agentic -> [
  leaven-kernel,
  leaven-core,
  leaven-surface,
  leaven-store,
  leaven-workspace,
  leaven-engine,
  leaven-agent,
  leaven-render
]

leaven-agentic-skill -> [
  leaven-kernel,
  leaven-core,
  leaven-workspace,
  leaven-engine,
  leaven-agent,
  leaven-agentic,
  leaven-artifact-skill
]
```

### Optimizers

```text
leaven-gepa -> [
  leaven-kernel,
  leaven-core,
  leaven-surface,
  leaven-engine,
  leaven-evidence,
  leaven-preference,
  leaven-population,
  leaven-render,
  leaven-lm
]

leaven-mipro -> [
  leaven-kernel,
  leaven-core,
  leaven-engine,
  leaven-evidence,
  leaven-population
]

leaven-textgrad -> [
  leaven-kernel,
  leaven-core,
  leaven-surface,
  leaven-engine,
  leaven-evidence,
  leaven-population,
  leaven-lm
]

leaven-trace -> [
  leaven-kernel,
  leaven-core,
  leaven-engine,
  leaven-evidence,
  leaven-render,
  leaven-lm
]
```

### Domain adapters

```text
leaven-dsrs -> [
  leaven-kernel,
  leaven-core,
  leaven-surface,
  leaven-engine,
  leaven-lm
]

leaven-cuda -> [
  leaven-kernel,
  leaven-core,
  leaven-engine,
  leaven-store,
  leaven-workspace
]

leaven-python -> [
  leaven-kernel,
  leaven-core,
  leaven-engine
]
```

### Umbrella

```text
leaven -> [
  leaven-kernel,
  leaven-core,
  leaven-surface,
  leaven-engine,
  leaven-derive,
  leaven-std,
  leaven-gepa,
  leaven-workspace,
  leaven-agentic
]
```

### Forbidden edges

```text
leaven-core -> leaven-surface
leaven-core -> leaven-engine
leaven-core -> leaven-workspace
leaven-core -> leaven-store
leaven-core -> leaven-gepa

leaven-surface -> leaven-engine
leaven-surface -> leaven-workspace
leaven-surface -> leaven-gepa
leaven-surface -> leaven-artifact-git
leaven-surface -> leaven-artifact-jj

leaven-store -> leaven-engine
leaven-store -> concrete storage backends

leaven-workspace -> leaven-core
leaven-workspace -> leaven-engine
leaven-workspace -> leaven-surface

leaven-engine -> leaven-gepa
leaven-engine -> leaven-std
leaven-engine -> leaven-population
leaven-engine -> leaven-preference
leaven-engine -> concrete backend crates
leaven-engine -> concrete LLM SDKs

leaven-artifacts -> leaven-workspace
leaven-artifacts -> leaven-engine

leaven-lm -> leaven-gepa
leaven-lm -> concrete LLM SDKs

leaven-agent -> leaven-core
leaven-agent -> leaven-engine
leaven-agent -> leaven-gepa

leaven-std -> leaven-gepa
leaven-derive -> leaven-engine
```

---

## 4. `leaven-kernel`

### Contract

Universal mechanical primitives. No optimizer semantics.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Universal mechanical primitives for Leaven.
//!
//! No optimizer semantics live here.

pub mod cost;
pub mod error;
pub mod finite;
pub mod fingerprint;
pub mod ids;
pub mod metadata;
pub mod time;

pub use cost::{
    Amount, AmountError, Budget, BudgetExceeded, BudgetSnapshot, Cost, CostAxis,
    CostUnit, Metered,
};

pub use error::{
    ErrorKind, ErrorRecord, IntoErrorRecord, Retryability,
};

pub use finite::{
    FiniteF64, FiniteF64Error,
};

pub use fingerprint::{
    Fingerprint, FingerprintBuilder,
};

pub use ids::{
    ApplyAttemptId, AssessmentId, BlobRef, CandidateId, CaseId, CheckpointId,
    ContentId, EvaluationRequestId, EvaluationSetId, EvaluatorId, EvidenceRef,
    IterationId, PopulationId, ProposalBatchId, ProposalId, ProposerId,
    RenderId, RendererId, ResolvedEvaluationSetId, RunId, StageId, StopperId,
};

pub use metadata::{
    MetadataBag, MetadataKey, MetadataValue,
};

pub use time::{
    Timestamp, now,
};

pub mod prelude {
    pub use crate::{
        Amount, AmountError, Budget, BudgetExceeded, BudgetSnapshot, Cost,
        CostUnit, Metered, ErrorKind, ErrorRecord, FiniteF64,
        FiniteF64Error, Fingerprint, MetadataBag,
    };
    pub use crate::ids::*;
}
```

---

## 5. `leaven-core`

### Contract

Cold optimizer algebra: artifact, proposal, evaluation, evidence, problem. No components, surfaces, rendering, graph storage, workspaces, or GEPA.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Cold optimizer algebra.
//!
//! This crate defines the core states that can exist in a Leaven optimization
//! run. It does not define an engine, graph storage, surfaces, renderers,
//! workspaces, populations, or GEPA.

pub mod artifact;
pub mod evidence;
pub mod evaluation;
pub mod preference;
pub mod problem;
pub mod proposal;

pub use artifact::{
    Artifact, ArtifactIdentity, CacheIdentified, CacheIdentity,
};

pub use evidence::{
    Evidence,
};

pub use evaluation::{
    Assessment, AssessmentGranularity, AssessmentTarget, CaseSetVersion,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, PairOrder,
    PartitionId, ResolvedEvaluationRequest, ResolvedEvaluationSet,
    ResolvedRequestKind, Tag, Window,
};

pub use preference::{
    Preference,
};

pub use problem::{
    OptimizationProblem,
};

pub use proposal::{
    CausalInputs, ExternalRef, InfoRef, Proposal, ProposalBatch,
    ProposalBatchSemantics, ProposalBuilder, ProposalEffect,
    ProposalEffectKind, ProposalProvenance,
};

pub mod prelude {
    pub use crate::{
        Artifact, ArtifactIdentity, Assessment, AssessmentGranularity,
        AssessmentTarget, CacheIdentified, CacheIdentity, CausalInputs, EvaluationRequest,
        EvaluationSet, Evidence, InfoRef, OptimizationProblem, Preference,
        Proposal, ProposalBatch, ProposalBatchSemantics, ProposalEffect,
        ProposalProvenance,
    };
}
```

### `src/artifact.rs`

```rust
//! Artifact contract.

use leaven_kernel::ContentId;

/// Domain value being optimized.
///
/// An artifact is opaque to the framework except for identity and change
/// application.
pub trait Artifact: Clone + Send + Sync + 'static {
    type Change: Clone + Send + Sync + 'static;
    type ApplyError: std::error::Error + Send + Sync + 'static;

    /// Stable identity of this artifact state.
    ///
    /// This may be content-addressed or external. Deterministic evaluation
    /// caching uses the separate `CacheIdentified` capability.
    fn identity(&self) -> ArtifactIdentity;

    /// Optional validity check for newly-created artifacts.
    fn validate(&self) -> Result<(), Self::ApplyError> {
        Ok(())
    }

    /// Apply a typed change.
    ///
    /// Law: pure/functional; failure does not mutate the original artifact.
    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError>;
}

/// Artifact identity.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ArtifactIdentity {
    /// Collision-resistant content identity.
    Content(ContentId),

    /// Stable external identity for graph lineage. Not automatically cache-safe.
    External(ExternalRef),
}

/// Stronger capability for deterministic evaluator caching.
pub trait CacheIdentified: Artifact {
    fn cache_identity(&self) -> Option<CacheIdentity>;
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CacheIdentity {
    Content(ContentId),
    ExternalContent(ExternalRef),
    User(leaven_kernel::Fingerprint),
}
```

### `src/evidence.rs`

```rust
//! Evidence marker.
//!
//! Evidence is opaque to core. Optional evidence capability traits live in
//! `leaven-evidence`, not here.

pub trait Evidence: Send + Sync + 'static {}
```

### `src/preference.rs`

```rust
//! Preference result.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Preference {
    LeftBetter,
    RightBetter,
    Equivalent,
    Incomparable,
}
```

### `src/problem.rs`

```rust
//! Run-associated type bundle.

use crate::{Artifact, Evidence};

pub trait OptimizationProblem: Send + Sync + 'static {
    type Artifact: Artifact;
    type Case: Send + Sync + 'static;
    type Evidence: Evidence;
    type ProposalAnnotations: Clone + Send + Sync + 'static;
}
```

### `src/proposal.rs`

```rust
//! Proposals and proposal batches.

use leaven_kernel::{
    AssessmentId, CandidateId, MetadataBag, ProposalId,
};

use crate::OptimizationProblem;

#[derive(Clone, Debug)]
pub struct Proposal<P: OptimizationProblem> {
    pub effect: ProposalEffect<P>,
    pub provenance: ProposalProvenance,
    pub annotations: P::ProposalAnnotations,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug)]
pub enum ProposalEffect<P: OptimizationProblem> {
    Create {
        artifact: P::Artifact,
    },
    Change {
        target: CandidateId,
        change: <P::Artifact as crate::Artifact>::Change,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalEffectKind {
    Create,
    Change,
}

#[derive(Clone, Debug)]
pub struct ProposalProvenance {
    pub causal: CausalInputs,
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
    Assessment(AssessmentId),
    Proposal(ProposalId),
    External(ExternalRef),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ExternalRef {
    pub kind: String,
    pub id: String,
}

#[derive(Clone, Debug)]
pub struct ProposalBatch<P: OptimizationProblem> {
    pub proposals: Vec<Proposal<P>>,
    pub semantics: ProposalBatchSemantics,
    pub metadata: MetadataBag,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalBatchSemantics {
    Alternatives,
    CandidatePool,
}

pub struct ProposalBuilder<P: OptimizationProblem> {
    proposal: Proposal<P>,
}

// builder impls omitted here; generated in actual crate
```

### `src/evaluation.rs`

```rust
//! Evaluation requests and assessments.

use leaven_kernel::{
    CandidateId, CaseId, Cost, EvaluationSetId, MetadataBag,
    ResolvedEvaluationSetId,
};

use crate::OptimizationProblem;

#[derive(Clone, Debug)]
pub enum EvaluationSet {
    Unscoped,
    All,
    Partition(PartitionId),
    Cases(Vec<CaseId>),
    Tagged(Tag),
    Recent { window: Window },
    Sample { of: Box<Self>, n: usize, seed: u64 },
    Stratified { of: Box<Self>, by: Tag, k: usize, seed: u64 },
    Union(Vec<Self>),
    Intersect(Vec<Self>),
    Difference(Box<Self>, Box<Self>),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PartitionId(pub smol_str::SmolStr);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Tag(pub smol_str::SmolStr);

#[derive(Clone, Debug)]
pub struct Window {
    pub limit: usize,
}

#[derive(Clone, Debug)]
pub struct ResolvedEvaluationSet {
    pub id: ResolvedEvaluationSetId,
    pub expr: EvaluationSet,
    pub case_ids: Vec<CaseId>,
    pub resolved_at: chrono::DateTime<chrono::Utc>,
    pub case_set_version: CaseSetVersion,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct CaseSetVersion(pub String);

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
pub struct ResolvedEvaluationRequest {
    pub kind: ResolvedRequestKind,
    pub set: ResolvedEvaluationSet,
    pub granularity: AssessmentGranularity,
    pub purpose: EvaluationPurpose,
}

#[derive(Clone, Debug)]
pub enum ResolvedRequestKind {
    Independent { candidates: Vec<CandidateId> },
    Pairwise { left: CandidateId, right: CandidateId, order: PairOrder },
    Listwise { candidates: Vec<CandidateId> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssessmentGranularity {
    Aggregate,
    PerCase,
    Both,
}

#[derive(Clone, Debug)]
pub enum EvaluationPurpose {
    SeedBaseline,
    Feedback,
    Screening,
    Search,
    Validation,
    FinalTest,
    Selection,
    Probe,
    Custom(smol_str::SmolStr),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairOrder {
    Ordered,
    Unordered,
}

#[derive(Clone, Debug)]
pub enum AssessmentTarget {
    Unscoped,
    EvaluationSet(EvaluationSetId),
    Case { set: EvaluationSetId, case: CaseId },
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

---

## 6. `leaven-surface`

### Contract

Explicit edit/read surfaces over artifacts. This is where “component-like” semantics live.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Explicit edit/read surfaces over artifacts.
//!
//! An artifact is intrinsic. A surface is a chosen projection over an artifact.
//! Repositories, directories, structs, and compound systems may have many
//! surfaces or no useful surface at all.

pub mod address;
pub mod edit_surface;
pub mod error;
pub mod part;
pub mod path_surface;
pub mod selection;

pub use address::{
    PartAddress,
};

pub use edit_surface::{
    EditSurface, SurfaceFingerprint,
};

pub use error::{
    SurfaceError,
};

pub use part::{
    Part, PartView,
};

pub use path_surface::{
    PathAddress, PathPartId, PathSurfaceConfig,
};

pub use selection::{
    PartSelection,
};

pub mod prelude {
    pub use crate::{
        EditSurface, Part, PartAddress, PartSelection, PartView, SurfaceError,
        SurfaceFingerprint,
    };
}
```

### `src/edit_surface.rs`

```rust
use leaven_core::Artifact;

use crate::{Part, SurfaceError, SurfaceFingerprint};

/// Chosen projection/edit surface over an artifact.
///
/// This is not an intrinsic property of the artifact. It is a lens selected by
/// an optimizer, proposer, renderer, or domain adapter.
pub trait EditSurface<A: Artifact>: Send + Sync {
    type PartId: Eq + std::hash::Hash + Clone + Send + Sync + 'static;
    type Address: Eq + std::hash::Hash + Clone + Send + Sync + 'static;
    type View<'a>: Send + Sync
    where
        A: 'a;
    type Edit: Clone + Send + Sync + 'static;

    /// Stable identity of this surface definition.
    ///
    /// Change this when layout rules, filters, parsing, ID extraction, or
    /// ignored-file logic change.
    fn fingerprint(&self) -> SurfaceFingerprint;

    fn parts<'a>(
        &self,
        artifact: &'a A,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError>;

    fn change_part(
        &self,
        artifact: &A,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<A::Change, SurfaceError>;
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SurfaceFingerprint(pub leaven_kernel::Fingerprint);
```

### `src/part.rs`

```rust
pub struct Part<Id, Address, View> {
    pub id: Id,
    pub address: Address,
    pub view: View,
}

pub struct PartView<T> {
    pub inner: T,
}

pub enum PartSelection<Id = PartAddress> {
    All,
    Only(Vec<Id>),
}
```

### Surface laws

```text
Surface identity is scoped to the surface, not the artifact.
Part IDs are scoped to `SurfaceFingerprint`.
Consumers must not combine `S::PartId` values with evidence generated under a
different surface fingerprint.
Path-based surfaces preserve path identity only; rename is remove + add.
Logical-ID surfaces may preserve identity across rename if ID extraction says so.
SurfaceFingerprint changes when interpretation changes.
change_part produces artifact-native Change; it does not mutate artifacts.
Borrowed `View<'a>` values are inspection-only; async stages should convert them
to owned request/rendering data before `.await`.
```

---

## 7. `leaven-evidence`

### Contract

Standard evidence shapes and optional evidence capability traits. Generic, not component-shaped.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Standard evidence shapes and optional evidence capabilities.

pub mod attribution;
pub mod casewise;
pub mod command;
pub mod diff;
pub mod json;
pub mod listwise;
pub mod mixed;
pub mod pairwise;
pub mod scalar;
pub mod score_vector;
pub mod string;

pub use attribution::{
    AttributableEvidence, Attribution, AttributionDomain, AttributionKey,
};

pub use casewise::{
    CaseOutcome, CasewiseEvidence,
};

pub use command::{
    CommandEvidence, CommandRecord,
};

pub use diff::{
    DiffEvidence, RenderedDiff,
};

pub use json::JsonEvidence;
pub use listwise::{ListwiseRankingEvidence, RankingItem};
pub use mixed::MixedEvidence;
pub use pairwise::{PairwiseJudgment, PairwiseJudgmentEvidence};
pub use scalar::ScalarEvidence;
pub use score_vector::{
    Direction, RawScoreValue, ScoreAxis, ScorePoint, ScoreVectorEvidence,
};
pub use string::StringEvidence;

pub mod prelude {
    pub use crate::{
        AttributableEvidence, CaseOutcome, CasewiseEvidence, CommandEvidence,
        DiffEvidence, Direction, JsonEvidence, ListwiseRankingEvidence, MixedEvidence,
        PairwiseJudgmentEvidence, ScalarEvidence, ScoreVectorEvidence,
        StringEvidence,
    };
}
```

### `src/casewise.rs`

```rust
use leaven_core::Evidence;
use leaven_kernel::{CaseId, FiniteF64};

/// Evidence that exposes per-case measured outcomes.
pub trait CasewiseEvidence: Evidence {
    fn case_outcome(&self, case: CaseId) -> Option<CaseOutcome>;
    fn case_outcomes(&self) -> Vec<(CaseId, CaseOutcome)>;
}

#[derive(Clone, Debug)]
pub struct CaseOutcome {
    pub score: Option<FiniteF64>,
    pub passed: Option<bool>,
}
```

### `src/attribution.rs`

```rust
use leaven_core::Evidence;
use leaven_kernel::FiniteF64;

/// Evidence that attributes behavior to arbitrary keys.
///
/// Keys may be surface part IDs, paths, agents, changesets, tools, modules,
/// conflict regions, or any user-defined key.
pub trait AttributableEvidence<K>: Evidence {
    fn attribution_domain(&self) -> AttributionDomain;
    fn attributions(&self) -> Vec<Attribution<K>>;

    fn evidence_for(&self, key: &K) -> Option<String>;
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AttributionDomain {
    Surface(leaven_kernel::Fingerprint),
    ToolCalls,
    Agents,
    Changesets,
    User(leaven_kernel::Fingerprint),
}

#[derive(Clone, Debug)]
pub struct Attribution<K> {
    pub key: K,
    pub weight: Option<FiniteF64>,
    pub note: Option<String>,
}

pub trait AttributionKey: Eq + std::hash::Hash + Clone + Send + Sync + 'static {}
impl<T> AttributionKey for T where T: Eq + std::hash::Hash + Clone + Send + Sync + 'static {}
```

This is the corrected replacement for `ComponentEvidence`.

---

## 8. `leaven-store`

### Contract

Storage contracts only. No graph.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Persistence contracts for Leaven.
//!
//! This crate defines storage traits. Concrete storage backends live in backend
//! crates. This crate does not know RunGraph.

pub mod blob;
pub mod checkpoint;
pub mod evidence;
pub mod error;

pub use blob::{BlobStore, BlobWrite};
pub use checkpoint::{CheckpointBytes, CheckpointStore};
pub use evidence::EvidenceStore;
pub use error::StoreError;

pub mod prelude {
    pub use crate::{
        BlobStore, BlobWrite, CheckpointBytes, CheckpointStore,
        EvidenceStore, StoreError,
    };
}
```

---

## 9. `leaven-workspace`

### Contract

Workspace/sandbox substrate only. No artifacts, surfaces, engine, or optimizer semantics.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Workspace substrate for side-effectful stages.

pub mod command;
pub mod config;
pub mod error;
pub mod factory;
pub mod path;
pub mod policy;
pub mod view;
pub mod workspace;

pub use command::{Command, CommandOutput, ExitStatus};
pub use config::WorkspaceConfig;
pub use error::{FactoryError, WorkspaceError};
pub use factory::WorkspaceFactory;
pub use path::{WorkspacePath, WorkspacePathError};
pub use policy::{FilesystemPolicy, NetworkPolicy};
pub use view::WorkspaceView;
pub use workspace::{Workspace, WorkspaceBackend};

pub mod prelude {
    pub use crate::{
        Command, CommandOutput, FactoryError, Workspace, WorkspaceBackend,
        WorkspaceConfig, WorkspaceError, WorkspaceFactory, WorkspacePath,
        WorkspacePathError, WorkspaceView,
    };
}
```

`Workspace` is the concrete Leaven lease handle. Backend crates implement
`WorkspaceFactory` and `WorkspaceBackend`; ordinary stage code does not implement
or receive backend-specific workspace types.

Public workspace APIs use `WorkspacePath`, a normalized relative path. They must
not accept host `PathBuf` or backend-specific absolute paths.

```rust
pub struct WorkspacePath { /* normalized relative path */ }

pub trait WorkspaceFactory: Send + Sync {
    async fn allocate(&self, cfg: WorkspaceConfig) -> Result<Workspace, FactoryError>;
}

pub trait WorkspaceBackend: Send + Sync {
    async fn write_file(
        &mut self,
        path: WorkspacePath,
        bytes: bytes::Bytes,
    ) -> Result<(), WorkspaceError>;

    async fn read_file(
        &mut self,
        path: WorkspacePath,
    ) -> Result<bytes::Bytes, WorkspaceError>;

    async fn list_files(&mut self, path: WorkspacePath) -> Result<Vec<WorkspacePath>, WorkspaceError>;
    async fn set_executable(&mut self, path: WorkspacePath, executable: bool) -> Result<(), WorkspaceError>;
    async fn is_executable(&mut self, path: WorkspacePath) -> Result<bool, WorkspaceError>;

    async fn run_command(&mut self, cmd: Command) -> Result<CommandOutput, WorkspaceError>;
    async fn cleanup(self: Box<Self>) -> Result<(), WorkspaceError>;
    fn mark_abandoned(self: Box<Self>) {}
    fn local_mount(&self) -> Option<&std::path::Path> { None }
}
```

---

## 10. `leaven-engine`

### Contract

Run execution, graph, contexts, stage traits, budget, trust, cache, callbacks. Graph mutation is private.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Run engine for Leaven.
//!
//! External code cannot mutate RunGraph directly. All mutation goes through
//! RunContext.

mod budget;
mod cache;
mod case_set;
mod context;
mod engine;
mod events;
mod graph;
mod reports;
mod stage;
mod trust;

pub use budget::{BudgetHandle, BudgetLedger};

pub use cache::{
    CachePolicy, CacheStatus, EvaluationCache, EvaluationCacheKey,
};

pub use case_set::{
    CaseSet, CaseSetBuilder,
};

pub use context::{
    EvaluationContext, MaterializeContext, ProposalContext, RenderContext,
    RunContext,
};

pub use engine::{
    Engine, EngineBuilder, RunPersistence, RunResult, optimize,
};

pub use events::{
    CausalInputsSummary, ErrorPolicy, EvaluationRequestSummary,
    ProposalEffectKind, RunEvent, StopReason,
};

pub use graph::{
    AssessmentQuery, AssessmentView, CandidateTree, CandidateView,
    EvaluationRequestView, FailureRef, Lineage, ProposalBatchView,
    ProposalView, RunGraph, RunGraphView,
};

pub use reports::{
    ApplyOneReport, ApplyOutcome, ApplyReport, EvaluationReport,
    ProposalBatchReport,
};

pub use stage::{
    Arity, Callback, CandidateSelection, DynCallback, DynEvaluator, DynPreferenceRelation,
    DynProposer, DynStopper, Evaluator, Optimizer, Population,
    Materializer, MaterializationReport, PopulationEvent, PopulationView,
    PreferenceRelation, Proposer, Renderer, SelectionContext, SelectionError,
    SelectionOutcome, SelectionRationale, Stopper,
};

pub use trust::{
    Actor, EvalHandle, EvidenceVisibility, ProbeRecorder, ReadScope,
    TrustPolicy, TrustViolation,
};

pub mod prelude {
    pub use crate::{
        optimize, Arity, BudgetHandle, BudgetLedger, CachePolicy, Callback,
        CandidateSelection, Engine, EngineBuilder, EvaluationContext, Evaluator,
        Optimizer, MaterializeContext, Materializer, Population,
        PreferenceRelation, ProposalContext, Proposer, ReadScope, RenderContext,
        Renderer, RunContext, RunEvent, RunGraphView, RunResult,
        SelectionContext, Stopper, TrustPolicy,
    };
}
```

### Internal module layout

```text
graph/
  mod.rs
  storage.rs       private records
  indices.rs       private derived indices
  writer.rs        pub(crate) mutation surface
  view.rs          public views
  query.rs         public query builders

context/
  mod.rs
  run.rs
  propose.rs       pub(crate) implementation
  apply.rs         pub(crate) implementation
  evaluate.rs      pub(crate) implementation
  render.rs        pub(crate) implementation
  trust.rs         pub(crate) implementation

stage/
  mod.rs
  optimizer.rs
  evaluator.rs
  proposer.rs
  renderer.rs
  population.rs
  preference.rs
  callback.rs
  stopper.rs
```

### `src/stage/proposer.rs`

```rust
use futures::future::BoxFuture;
use leaven_core::{OptimizationProblem, ProposalBatch};
use leaven_kernel::{Metered, ProposerId};

pub trait Proposer<P: OptimizationProblem>: Send + Sync {
    type Request: Send + Sync;

    fn id(&self) -> ProposerId;

    /// Parent-shape hint for optimizers that perform parent selection.
    ///
    /// This is not a law on every emitted proposal.
    fn arity(&self) -> Arity;

    async fn propose(
        &self,
        request: Self::Request,
        ctx: crate::ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arity {
    None,
    Single,
    Pair,
    Variadic,
}

pub trait DynProposer<P: OptimizationProblem>: Send + Sync {
    fn id(&self) -> ProposerId;

    fn arity(&self) -> Arity;

    fn propose_boxed<'a>(
        &'a self,
        request: Box<dyn std::any::Any + Send>,
        ctx: crate::ProposalContext<'a, P>,
    ) -> BoxFuture<'a, Result<Metered<ProposalBatch<P>>, ProposalError>>;
}

#[derive(Debug, thiserror::Error)]
pub enum ProposalError {
    #[error("proposal failed: {0}")]
    Message(String),
}
```

### `src/stage/renderer.rs`

```rust
use leaven_core::OptimizationProblem;
use leaven_kernel::Metered;

pub trait Renderer<P: OptimizationProblem, T, Target>: Send + Sync {
    type View;

    async fn render(
        &self,
        value: &T,
        target: Target,
        ctx: crate::RenderContext<'_, P>,
    ) -> Result<Metered<Self::View>, crate::RenderError>;
}

pub trait Materializer<P: OptimizationProblem, T>: Send + Sync {
    async fn materialize_into(
        &self,
        value: &T,
        workspace: &mut leaven_workspace::WorkspaceView<'_>,
        ctx: crate::MaterializeContext<'_, P>,
    ) -> Result<Metered<MaterializationReport>, crate::MaterializeError>;
}

pub struct MaterializationReport {
    pub files_written: usize,
    pub bytes_written: u64,
    pub truncations: Vec<TruncationNote>,
}

pub struct TruncationNote {
    pub path: Option<leaven_workspace::WorkspacePath>,
    pub reason: String,
}

// No DynRenderer or DynMaterializer in v0.2.2. Define erasure only once the
// value/target/view contract is real and covered by stage trait contract tests.
// Do not ship empty public marker traits or a universal Rendered enum.
```

### `src/stage/population.rs`

```rust
use leaven_core::OptimizationProblem;
use leaven_kernel::{AssessmentId, CandidateId, PopulationId};

pub trait Population<P: OptimizationProblem>: Send {
    fn id(&self) -> PopulationId;

    fn insert_seed(
        &mut self,
        candidate: CandidateId,
        graph: crate::RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent>;

    fn observe_candidate(
        &mut self,
        _candidate: CandidateId,
        _graph: crate::RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        Vec::new()
    }

    fn observe_assessment(
        &mut self,
        _assessment: AssessmentId,
        _graph: crate::RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        Vec::new()
    }

    fn best(&self, graph: crate::RunGraphView<'_, P>) -> Option<CandidateId>;

    fn view<'a>(&'a self, graph: crate::RunGraphView<'a, P>) -> PopulationView<'a, P>;
}

pub struct PopulationView<'a, P: OptimizationProblem> {
    pub id: PopulationId,
    pub candidates: &'a [CandidateId],
    pub frontier: FrontierView<'a>,
    pub scores: ScoreView<'a, P>,
    pub niches: Option<NicheView<'a>>,
    pub selection_stats: SelectionStatsView<'a>,
}

pub struct FrontierView<'a> { /* members, dominated, case/niche frontier views */ }
pub struct ScoreView<'a, P: OptimizationProblem> { /* graph-backed score queries */ }
pub struct NicheView<'a> { /* candidate -> niche assignments */ }
pub struct SelectionStatsView<'a> { /* attempts, successes, last selected */ }

pub struct SelectionContext<'a> { /* iteration, rng, budget snapshot, arity */ }
pub struct CandidateSelection { pub candidates: Vec<CandidateId>, /* rationale */ }
pub struct SelectionOutcome { /* selected, proposals, applied, admitted, rejected */ }
pub enum SelectionError { EmptyPopulation, UnsupportedArity, InsufficientCandidates, Message(String) }
pub enum SelectionRationale { /* pareto frequency, greedy, beam, niche, exploration, user */ }

#[derive(Clone, Debug)]
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
        weight: leaven_kernel::FiniteF64,
        reason: String,
    },
}
```

---

## 11. `leaven-derive`

### Contract

Derive macros for artifact identity and optionally edit surfaces.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]

//! Derive macros for Leaven artifacts and surfaces.

extern crate proc_macro;

mod artifact;
mod content_hash;
mod parse;
mod surface;

use proc_macro::TokenStream;

#[proc_macro_derive(Artifact, attributes(leaven, content_skip))]
pub fn derive_artifact(input: TokenStream) -> TokenStream {
    artifact::derive(input)
}

#[proc_macro_derive(CacheIdentified, attributes(leaven, cache_skip))]
pub fn derive_cache_identified(input: TokenStream) -> TokenStream {
    content_hash::derive(input)
}

/// Generates a struct-field edit surface for ordinary Rust structs.
///
/// This is not an intrinsic component surface. It is an explicit surface type.
#[proc_macro_derive(EditSurface, attributes(leaven_surface))]
pub fn derive_edit_surface(input: TokenStream) -> TokenStream {
    surface::derive(input)
}
```

This avoids one macro named `Optimize` doing too much. The umbrella crate can still re-export convenience macros or provide `#[derive(Optimize)]` later as a facade macro if repetition proves painful.

---

## 12. `leaven-artifacts`

### Contract

Standard artifact implementations. May provide surfaces for those artifacts. Does not know workspaces or engine.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Standard artifact implementations and their standard edit surfaces.
//!
//! This crate does not know about workspaces or the engine.

pub mod dir;
pub mod part_map;
pub mod text;

pub use dir::{
    DirArtifact, DirChange, DirPathSurface, FsOp,
};

pub use part_map::{
    PartId, PartMapArtifact, PartMapChange, PartMapSurface,
};

pub use text::{
    TextArtifact, TextChange, TextSurface,
};

pub mod prelude {
    pub use crate::{
        DirArtifact, DirChange, DirPathSurface, FsOp, PartId,
        PartMapArtifact, PartMapChange, PartMapSurface, TextArtifact,
        TextChange, TextSurface,
    };
}
```

`DirPathSurface` is honest:

```text
PartId = path
rename = remove + add
no logical identity continuity
```

---

## 13. `leaven-artifact-git`

### Contract

Git artifact implementations and git-specific surfaces.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Git-backed artifacts and git-specific edit surfaces.

pub mod artifact;
pub mod change;
pub mod diff;
pub mod error;
pub mod surface;

pub use artifact::{
    GitArtifact, GitArtifactIdentityMode,
};

pub use change::{
    FsOp, GitChange,
};

pub use diff::{
    GitDiff, GitDiffSummary,
};

pub use error::{
    GitArtifactError,
};

pub use surface::{
    GitAgentKitSurface, GitPathSurface, GitSkillFrontmatterSurface,
};
```

Surfaces here are explicit:

```text
GitPathSurface:
  PartId = PathBuf
  identity is path-based

GitSkillFrontmatterSurface:
  PartId = SkillId parsed from frontmatter
  Address = PathBuf
  rename continuity possible

GitAgentKitSurface:
  PartId = AgentKitPart enum
  Address = path or logical location
```

No claim that `GitArtifact` intrinsically has components.

---

## 14. `leaven-artifact-jj`

### Contract

jj artifact implementations and jj-specific surfaces.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! jj-backed artifacts and jj-specific edit surfaces.

pub mod artifact;
pub mod change;
pub mod conflict;
pub mod error;
pub mod operation_log;
pub mod surface;

pub use artifact::{
    JjArtifact, JjArtifactIdentityMode,
};

pub use change::{
    JjChange, JjOp,
};

pub use conflict::{
    ConflictRegion, ConflictRegionId,
};

pub use error::{
    JjArtifactError,
};

pub use operation_log::{
    OperationId, OperationSummary,
};

pub use surface::{
    JjConflictSurface, JjPathSurface, JjChangesetSurface,
};
```

Important surfaces:

```text
JjPathSurface:
  path-based file edits

JjConflictSurface:
  PartId = ConflictRegionId
  Address = file path + conflict span / jj conflict marker

JjChangesetSurface:
  PartId = ChangesetId
  Address = operation/log position or revset
```

This is why surfaces exist.

---

## 15. `leaven-render`

### Contract

Standard value renderers and workspace materializers. Not cold core. Stage-owned
renderers/materializers remain the normal composition pattern.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Standard value renderers and workspace materializers.
//!
//! Most stages should own their renderers/materializers directly. This crate
//! provides reusable implementations, not a mandatory global rendering system.
//! Materializers are for agentic/sandboxed workspace consumers; ordinary LM
//! provider calls use value renderers.

pub mod graph;
pub mod history;
pub mod prompt;
pub mod surface;
pub mod workspace;

pub use graph::{
    CandidateTreeHtmlRenderer, RunGraphDebugRenderer,
};

pub use history::{
    LineageSummaryRenderer,
};

pub use prompt::{
    ReflectionPromptRenderer, StructuredPromptRenderer,
};

pub use surface::{
    SurfaceDiffRenderer, SurfacePartsRenderer,
};

pub use workspace::{
    ArtifactMaterializer, HistoryMaterializer,
    SurfaceMaterializer,
};

pub mod prelude {
    pub use crate::{
        ArtifactMaterializer, CandidateTreeHtmlRenderer,
        HistoryMaterializer, LineageSummaryRenderer,
        ReflectionPromptRenderer, StructuredPromptRenderer,
        SurfaceDiffRenderer, SurfacePartsRenderer, SurfaceMaterializer,
    };
}
```

This crate depends on engine because `Renderer` and `Materializer` traits live there.

---

## 16. `leaven-preference`

### Contract

Stateless preference relations.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Stateless preference relations over graph evidence.

pub mod borda;
pub mod copeland;
pub mod lexicographic;
pub mod pareto;
pub mod scalar;

pub use borda::BordaPreference;
pub use copeland::CopelandPreference;
pub use lexicographic::LexicographicPreference;
pub use pareto::ParetoPreference;
pub use scalar::{HigherScoreIsBetter, LowerScoreIsBetter};

pub mod prelude {
    pub use crate::{
        BordaPreference, CopelandPreference, HigherScoreIsBetter,
        LexicographicPreference, LowerScoreIsBetter, ParetoPreference,
    };
}
```

No Bradley-Terry here. Fitted preference state lives in populations.

---

## 17. `leaven-population`

### Contract

Standard populations/frontiers/live states.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Standard populations and frontiers.

pub mod beam;
pub mod keep_best;
pub mod lenient_pareto;
pub mod map_elites;
pub mod niche;
pub mod no_population;
pub mod novelty;
pub mod pareto_frontier;
pub mod tournament;

pub use beam::BeamPopulation;
pub use keep_best::KeepBest;
pub use lenient_pareto::LenientParetoFrontier;
pub use map_elites::MapElites;
pub use niche::NicheDescriptor;
pub use no_population::NoPopulation;
pub use novelty::NoveltyPopulation;
pub use pareto_frontier::{ParetoFrontier, ParetoFrontierBuilder};
pub use tournament::{
    BradleyTerryFit, PlackettLuceFit, TournamentConfig, TournamentPopulation,
};

pub mod prelude {
    pub use crate::{
        BeamPopulation, BradleyTerryFit, KeepBest, LenientParetoFrontier,
        MapElites, NicheDescriptor, NoPopulation, NoveltyPopulation,
        ParetoFrontier, PlackettLuceFit, TournamentPopulation,
    };
}
```

---

## 18. `leaven-lm`

### Contract

Provider-neutral LLM calls.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Provider-neutral LLM interface.

pub mod completion;
pub mod error;
pub mod messages;
pub mod model;
pub mod usage;

pub use completion::{Completion, CompletionBatch};
pub use error::LmError;
pub use messages::{Message, Messages, Role};
pub use model::{Lm, SamplingOptions};
pub use usage::TokenUsage;

pub mod prelude {
    pub use crate::{
        Completion, CompletionBatch, Lm, LmError, Message, Messages,
        Role, SamplingOptions, TokenUsage,
    };
}
```

Backends:

```rust
// leaven-lm-openai/src/lib.rs
#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! OpenAI LLM adapter.
pub mod client;
pub mod config;
pub use client::OpenAiLm;
pub use config::OpenAiConfig;
```

```rust
// leaven-lm-anthropic/src/lib.rs
#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! Anthropic LLM adapter.
pub mod client;
pub mod config;
pub use client::AnthropicLm;
pub use config::AnthropicConfig;
```

```rust
// leaven-lm-local/src/lib.rs
#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! Local/server-backed LLM adapter.
pub mod client;
pub mod config;
pub use client::LocalLm;
pub use config::LocalLmConfig;
```

```rust
// leaven-lm-mock/src/lib.rs
#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! Deterministic mock LLM for tests.
pub mod mock;
pub use mock::{MockLm, MockLmScript};
```

---

## 19. `leaven-agent`

### Contract

Provider-neutral agent runtime interface over workspaces. The detailed runtime
contract lives in `docs/specs/agentic_stage_runtime.md`.

`leaven-agent` executes one session inside an already-materialized workspace.
It must not depend on `leaven-core`, `leaven-engine`, `leaven-gepa`, or any
optimizer crate.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Provider-neutral agent runtime interface.

pub mod error;
pub mod fake;
pub mod runtime;
pub mod session;
pub mod transcript;

pub use error::AgentRuntimeError;
pub use fake::{FakeAgentAction, FakeAgentRuntime};
pub use runtime::AgentRuntime;
pub use session::{
    AgentContextRef, AgentInstructions, AgentLimits, AgentRunContext,
    AgentRunRequest, AgentRuntimeCapabilities, AgentSession, AgentStatus,
    AgentToolPolicy, CancellationRef, JsonSchemaRef, OutputContract,
    WorkspaceAccessMode,
};
pub use transcript::{
    AgentTranscript, CommandRecord, RawProviderEvent, ToolCallRecord,
    TranscriptEvent, TranscriptRole, WorkspaceReadRecord,
};

pub mod prelude {
    pub use crate::{
        AgentContextRef, AgentInstructions, AgentLimits, AgentRunContext,
        AgentRunRequest, AgentRuntime, AgentRuntimeCapabilities,
        AgentRuntimeError, AgentSession, AgentStatus, AgentToolPolicy,
        AgentTranscript, CancellationRef, CommandRecord, FakeAgentAction,
        FakeAgentRuntime, JsonSchemaRef, OutputContract, RawProviderEvent,
        ToolCallRecord, TranscriptEvent, TranscriptRole, WorkspaceAccessMode,
        WorkspaceReadRecord,
    };
}
```

Backends:

```rust
// leaven-agent-claude-code/src/lib.rs
#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! Claude Code agent runtime adapter.
pub mod config;
pub mod runtime;
pub mod transcript;
pub use config::ClaudeCodeConfig;
pub use runtime::ClaudeCodeRuntime;
```

```rust
// leaven-agent-codex/src/lib.rs
#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! Codex agent runtime adapter.
//!
//! Provider-adapter contract:
//! `docs/specs/codex_app_server_agent_runtime.md`.
pub mod config;
pub mod runtime;
pub use config::CodexConfig;
pub use runtime::CodexRuntime;
```

```rust
// leaven-agent-opencode/src/lib.rs
#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! OpenCode agent runtime adapter.
pub mod config;
pub mod runtime;
pub use config::OpenCodeConfig;
pub use runtime::OpenCodeRuntime;
```

---

## 20. `leaven-agentic`

### Contract

Agentic stage helpers that connect agent runtimes, workspaces, value renderers,
materializers, and engine stage traits. This is the primary consumer of
`Materializer`; vanilla `leaven-lm` calls should not need workspace
materialization.

This crate is allowed to know both Leaven stage traits and provider-neutral
agent sessions. It owns the conversion from agent outputs into `ProposalBatch`
or `Assessment` values.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Agentic stage helpers for Leaven.

pub mod error;
pub mod evaluator;
pub mod parser;
pub mod proposer;

pub use proposer::{
    AgenticProposer, AgenticProposerConfig,
};

pub use evaluator::{
    AgenticEvaluator, AgenticEvaluatorConfig,
};

pub use parser::{
    AgentPromptTarget, AgenticRunInput, EvaluationInputBuilder,
    EvidenceParser, ProposalParser,
};

pub use error::{AgenticAdapterError, AgenticParseError};

pub mod prelude {
    pub use crate::{
        AgentPromptTarget, AgenticAdapterError, AgenticEvaluator,
        AgenticEvaluatorConfig, AgenticParseError, AgenticProposer,
        AgenticProposerConfig, AgenticRunInput, EvaluationInputBuilder,
        EvidenceParser, ProposalParser,
    };
}
```

---

## 20.1 `leaven-agentic-skill`

### Contract

Skill-specific agentic helpers that connect `SkillBank` artifacts to the
generic `leaven-agentic` stage adapters. This crate owns skill-bank workspace
layouts, skill materializers, and proposal parsers that read an edited skill
workspace back into `SkillBankChange`.

It must not know provider protocols. Codex, Claude Code, OpenCode, and future
agent providers stay in `leaven-agent-*`; this crate only knows provider-neutral
agent sessions through `leaven-agentic` parser contracts.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Skill-specific agentic stage helpers.

mod diff;
mod input;
mod layout;
mod materializer;
mod parser;

pub use diff::SkillBankDiff;
pub use input::SkillBankProposalInput;
pub use layout::SkillWorkspaceLayout;
pub use materializer::SkillBankMaterializer;
pub use parser::SkillBankWorkspaceProposalParser;
```

---

## 21. `leaven-gepa`

### Contract

GEPA as one optimizer implementation over Leaven.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! GEPA optimizer implementation for Leaven.

pub mod batch;
pub mod candidate_selector;
pub mod gate;
pub mod gepa;
pub mod merge;
pub mod mutation;
pub mod part_selector;
pub mod prompt;
pub mod validation;

pub use batch::{
    BatchSampler, EpochShuffled, FixedMinibatch,
};

pub use candidate_selector::{
    BeamCandidateSelector, CandidateSelector,
    NicheWeighted, ParetoFrequencyWeighted, RoundRobinCandidate,
    SelectBestCandidate, UniformFrontier,
};

pub use gate::{
    Gate, GateDecision, ImprovementOrEqual, NoRegression, StrictImprovement,
};

pub use gepa::{
    Gepa, GepaBuilder, GepaConfig, GepaMerge, GepaMutationRequest,
    GepaProposal, GepaProposer,
};

pub use merge::{
    MergeScheduler, SystemAwareMerge,
};

pub use mutation::{
    ReflectiveMutation, ReflectiveMutationConfig,
};

pub use part_selector::{
    InvokedAndFailingPart, PartSelector, RoundRobinPart,
};

pub use validation::{
    FullValidation, MinibatchThenValidation, ValidationPolicy,
};

pub mod prelude {
    pub use crate::{
        BatchSampler, CandidateSelector, EpochShuffled, FullValidation, Gate,
        Gepa, GepaMerge, GepaMutationRequest, GepaProposal, GepaProposer,
        ImprovementOrEqual, MinibatchThenValidation,
        ParetoFrequencyWeighted, PartSelector, ReflectiveMutation,
        RoundRobinCandidate, RoundRobinPart, StrictImprovement,
        SystemAwareMerge, UniformFrontier, ValidationPolicy, InvokedAndFailingPart,
    };
}
```

GEPA's canonical shape is `Gepa<P, S, Pop = Box<dyn Population<P>>>` where
`S: EditSurface<P::Artifact>`. `S` stays static because `S::PartId`, `S::Edit`,
and `S::fingerprint()` connect part selection, trace attribution, and proposal
lowering. Generic GEPA proposers may emit surface edits; GEPA lowers them
through `S::change_part` into artifact-native changes before recording
`ProposalEffect::Change`.

GEPA keeps `Population` and `CandidateSelector` separate. Population owns
archive/frontier/admission/fitted-state; `CandidateSelector` chooses the next
candidate(s) from `PopulationView`.

```rust
pub trait CandidateSelector<P: OptimizationProblem>: Send {
    fn select(
        &mut self,
        population: leaven_engine::PopulationView<'_, P>,
        graph: leaven_engine::RunGraphView<'_, P>,
        ctx: leaven_engine::SelectionContext<'_>,
    ) -> Result<leaven_engine::CandidateSelection, leaven_engine::SelectionError>;

    fn observe_selection_outcome(&mut self, _outcome: &leaven_engine::SelectionOutcome) {}
}
```

GEPA now talks about `PartSelector` over an `EditSurface`, not artifact-intrinsic components.

---

## 22. Other optimizer crates

### `leaven-mipro/src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! MIPRO-style optimizer implementation.

pub mod acquisition;
pub mod bootstrap;
pub mod mipro;
pub mod surrogate;

pub use acquisition::{
    AcquisitionFunction, ExpectedImprovement, TpeAcquisition,
};

pub use bootstrap::{
    Bootstrapper, GroundedBootstrapper,
};

pub use mipro::{
    Mipro, MiproBuilder, MiproConfig,
};

pub use surrogate::{
    ObservationTable, SurrogateModel, TpeSurrogate,
};
```

### `leaven-textgrad/src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! TextGrad-style optimizer implementation.

pub mod aggregation;
pub mod optimizer;
pub mod update;

pub use aggregation::{
    FeedbackAggregator, PerPartFeedbackAggregator,
};

pub use optimizer::{
    TextGrad, TextGradBuilder, TextGradConfig,
};

pub use update::{
    TextGradientUpdater,
};
```

TextGrad uses:

```text
EditSurface<A>
AttributableEvidence<S::PartId>
```

not artifact components.

### `leaven-trace/src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Trace / OptoPrime-style optimizer helpers.

pub mod optimizer;
pub mod renderer;
pub mod subgraph;

pub use optimizer::{
    OptoPrime, OptoPrimeBuilder,
};

pub use renderer::{
    SubgraphAsCode, SubgraphAsCodeRenderer,
};

pub use subgraph::{
    ExecutionSubgraph, TraceNode,
};
```

---

## 23. Adapter crates

### `leaven-dsrs/src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! DSRs integration for Leaven.

pub mod artifact;
pub mod evaluator;
pub mod signature;
pub mod surface;

pub use artifact::{
    DsrsProgramArtifact, DsrsProgramChange,
};

pub use evaluator::{
    DsrsEvaluator,
};

pub use signature::{
    DsrsSignatureBridge,
};

pub use surface::{
    DsrsProgramSurface,
};
```

### `leaven-cuda/src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! CUDA/code-generation evaluation helpers.

pub mod artifact;
pub mod evidence;
pub mod evaluator;
pub mod profiler;
pub mod surface;

pub use artifact::{
    CudaKernelArtifact, CudaKernelChange,
};

pub use evidence::{
    CudaEvidence,
};

pub use evaluator::{
    CudaEvaluator,
};

pub use profiler::{
    CudaProfiler, KernelBenchRunner,
};

pub use surface::{
    CudaSourceSurface,
};
```

### `leaven-python/src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Python/PyO3 bridge for Leaven.

pub mod artifact;
pub mod bridge;
pub mod evaluator;

pub use artifact::{
    PyArtifact,
};

pub use bridge::{
    PyLeaven,
};

pub use evaluator::{
    PyEvaluator,
};
```

---

## 24. `leaven-std`

### Contract

Shallow facade over standard pieces. Not an implementation bucket.

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Curated standard library for Leaven.

pub mod artifacts {
    pub use leaven_artifacts::*;

    #[cfg(feature = "git")]
    pub use leaven_artifact_git::*;

    #[cfg(feature = "jj")]
    pub use leaven_artifact_jj::*;
}

pub mod evidence {
    pub use leaven_evidence::*;
}

pub mod preferences {
    pub use leaven_preference::*;
}

pub mod populations {
    pub use leaven_population::*;
}

pub mod render {
    pub use leaven_render::*;
}

pub mod surfaces {
    pub use leaven_surface::*;
}

pub mod prelude {
    pub use leaven_artifacts::prelude::*;
    pub use leaven_evidence::prelude::*;
    pub use leaven_preference::prelude::*;
    pub use leaven_population::prelude::*;
    pub use leaven_render::prelude::*;
    pub use leaven_surface::prelude::*;
}
```

---

## 25. Umbrella crate `leaven`

### `Cargo.toml` feature sketch

```toml
[features]
default = ["std", "derive", "gepa"]

std = ["dep:leaven-std"]
derive = ["dep:leaven-derive"]
gepa = ["dep:leaven-gepa"]

workspace = ["dep:leaven-workspace"]
agentic = ["workspace", "dep:leaven-agentic"]

git = ["dep:leaven-artifact-git", "leaven-std/git"]
jj = ["dep:leaven-artifact-jj", "leaven-std/jj"]

store-sqlite = ["dep:leaven-store-sqlite"]

workspace-local = ["workspace", "dep:leaven-workspace-local"]
workspace-docker = ["workspace", "dep:leaven-workspace-docker"]
workspace-e2b = ["workspace", "dep:leaven-workspace-e2b"]

lm-openai = ["dep:leaven-lm-openai"]
lm-anthropic = ["dep:leaven-lm-anthropic"]
```

### `src/lib.rs`

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Leaven: optimize anything in Rust.
//!
//! This is the umbrella crate. It is an import experience, not an
//! implementation crate.

pub mod prelude;

pub use leaven_core as core;
pub use leaven_engine as engine;
pub use leaven_kernel as kernel;
pub use leaven_surface as surface;

pub use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity,
    AssessmentTarget, CacheIdentified, CacheIdentity, CausalInputs, EvaluationRequest,
    EvaluationSet, Evidence, InfoRef, OptimizationProblem, Preference,
    Proposal, ProposalBatch, ProposalBatchSemantics, ProposalEffect,
    ProposalProvenance,
};

pub use leaven_surface::{
    EditSurface, Part, PartAddress, PartSelection, SurfaceError,
    SurfaceFingerprint,
};

pub use leaven_engine::{
    optimize, Arity, CachePolicy, CandidateSelection, Engine, EngineBuilder,
    Evaluator, MaterializeContext, Materializer, Optimizer, Population,
    PreferenceRelation, ProposalContext, Proposer, ReadScope, Renderer,
    RunContext, RunEvent, RunGraphView, RunResult, SelectionContext, Stopper,
    TrustPolicy,
};

pub use leaven_kernel::{
    Amount, AmountError, Budget, BudgetSnapshot, CandidateId, ContentId, Cost,
    CostUnit, ErrorRecord, FiniteF64, FiniteF64Error, MetadataBag, ProposalId,
};

#[cfg(feature = "derive")]
pub use leaven_derive::{
    Artifact as DeriveArtifact,
    CacheIdentified as DeriveCacheIdentified,
    EditSurface as DeriveEditSurface,
};

#[cfg(feature = "std")]
pub use leaven_std as stdlib;

#[cfg(feature = "gepa")]
pub use leaven_gepa::Gepa;

#[cfg(feature = "workspace")]
pub use leaven_workspace as workspace;

#[cfg(feature = "agentic")]
pub use leaven_agentic as agentic;
```

### `src/prelude.rs`

```rust
//! Common imports for most Leaven users.

pub use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity,
    AssessmentTarget, CacheIdentified, CacheIdentity, EvaluationRequest, EvaluationSet,
    Evidence, OptimizationProblem, Proposal, ProposalBatch, ProposalEffect,
};

pub use leaven_surface::{
    EditSurface, Part, PartAddress, PartSelection,
};

pub use leaven_engine::{
    optimize, Arity, CachePolicy, CandidateSelection, Engine, Evaluator,
    MaterializeContext, Materializer, Optimizer, Population, PreferenceRelation,
    Proposer, Renderer, RunContext, RunGraphView, SelectionContext, Stopper,
    TrustPolicy,
};

pub use leaven_kernel::{
    Budget, CandidateId, ContentId, Cost, CostUnit, ErrorRecord, FiniteF64,
    MetadataBag,
};

#[cfg(feature = "derive")]
pub use leaven_derive::{
    Artifact as DeriveArtifact,
    CacheIdentified as DeriveCacheIdentified,
    EditSurface as DeriveEditSurface,
};

#[cfg(feature = "gepa")]
pub use leaven_gepa::prelude::*;

#[cfg(feature = "std")]
pub use leaven_std::prelude::*;
```

---

## 26. What New Code Lands Where

```text
new identity/cost/error/fingerprint/metadata primitive
  -> leaven-kernel

new artifact/proposal/evaluation/evidence algebra
  -> leaven-core

new artifact part/view/edit lens
  -> leaven-surface or artifact-specific crate

new graph/context/stage/trust/cache/event behavior
  -> leaven-engine

new storage contract
  -> leaven-store

new storage backend
  -> leaven-store-*

new workspace substrate capability
  -> leaven-workspace

new workspace backend
  -> leaven-workspace-*

new standard artifact
  -> leaven-artifacts

new git/jj-specific artifact or surface
  -> leaven-artifact-git / leaven-artifact-jj

new evidence shape or evidence capability
  -> leaven-evidence

new stateless preference relation
  -> leaven-preference

new population/frontier/fitted tournament model
  -> leaven-population

new reusable renderer
  -> leaven-render

new GEPA-specific part
  -> leaven-gepa

new optimizer algorithm
  -> leaven-<optimizer>

new LLM provider
  -> leaven-lm-<provider>

new agent runtime
  -> leaven-agent-<runtime>

new agentic stage helper
  -> leaven-agentic

new domain integration
  -> leaven-<domain>
```

---

## 27. Coverage of Required Cases

### GEPA

```text
leaven-gepa
leaven-surface
leaven-population
leaven-preference
leaven-evidence
leaven-lm
```

GEPA operates over an `EditSurface`, not artifact-intrinsic components.
Instance-pareto frontiers consume `CasewiseEvidence`; trace-aware part
selectors consume `AttributableEvidence<S::PartId>` with a matching surface
fingerprint.

### GEPA+Merge

```text
SystemAwareMerge` reads two candidates through graph + surface,
constructs artifact-native Change against one target,
records Pair causal inputs.
```

### MIPRO

```text
leaven-mipro
surrogate state private to optimizer
population optional / BeamPopulation
```

### TextGrad

```text
leaven-textgrad
requires EditSurface<A>
requires AttributableEvidence<S::PartId>
```

### Trace / OptoPrime

```text
leaven-trace
subgraph-as-code renderer
```

### MuF/Edit

```text
typed annotations in application or helper crate
ClaimsHeldGate in GEPA or user optimizer
```

### Git/JJ repos

```text
GitArtifact/JjArtifact are opaque artifacts.
GitPathSurface/JjConflictSurface/etc are explicit surfaces.
No cold-core component assumption.
```

### GSkill / Meta-Harness

```text
leaven-agentic
leaven-render
EvidenceStore
Materializer
ProposalEffect::Create for fresh artifacts
TrustPolicy hiding test partitions
```

---

## 28. No-Fuckery Checklist

Before implementation:

```text
□ leaven-core has no Decomposable / Component
□ leaven-core has no Workspace / Renderer / Store / Engine
□ leaven-surface has no Engine / Workspace / Git / JJ
□ leaven-artifacts has no Workspace / Engine
□ leaven-workspace has no Artifact / Surface / Engine
□ leaven-agent has no Core / Engine / GEPA / optimizer dependency
□ AgentRuntime does not mention OptimizationProblem / CandidateId / Proposal / Assessment
□ leaven-store has no RunGraph
□ leaven-engine has no GEPA / std population implementations / LLM SDKs
□ leaven-gepa depends on surface, not artifact components
□ TextGrad depends on surface + generic attribution, not components
□ metadata never stores causal parents or informed_by semantics
□ ProposalEffect::Create and Change are the only effects
□ RunGraph mutation methods are pub(crate)
□ reports are ID-only
□ Proposer::Request is associated
□ no `WorkspaceRenderer` export or alias; use `Materializer`
□ workspace file APIs use `WorkspacePath`, not host paths
□ GEPA candidate selection is a `CandidateSelector`, not hidden inside `Population`
□ no renderer/materializer registry in v0.2.2; stage-owned fields are normal
□ CI enforces dependency allowlist
```

---

## 29. Final Topology Thesis

```text
leaven-core defines optimizer algebra.
leaven-surface defines chosen projections over artifacts.
leaven-engine turns algebra into a run and owns graph mutation.
leaven-store persists large things without knowing the graph.
leaven-workspace provides sandbox/filesystem substrate without knowing artifacts.
standard crates provide reusable vocabulary.
optimizer crates implement algorithms.
edge crates hold heavy integrations.
leaven is only the import experience.
```

That is the corrected crate graph.
