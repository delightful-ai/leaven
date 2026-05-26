//! Typed identifiers used across a Leaven run.
//!
//! Every distinct kind of identity gets its own newtype so the type system —
//! not careful naming — keeps `CandidateId`s out of slots that expect
//! `ProposalId`s. The cost is a small amount of boilerplate; the payoff is
//! that mismatches are compile errors instead of silent bugs at the store or
//! graph boundary.
//!
//! Three identity shapes appear here:
//!
//! - **UUID-backed** ([`RunId`], [`CandidateId`], [`ProposalId`], etc.) —
//!   randomly generated, opaque, run-scoped. Stable for the lifetime of the
//!   run graph and serialize compactly.
//! - **Content-addressed** ([`ContentId`]) — a 32-byte hash treated as
//!   identity by the evaluation cache. See [`ContentId`] for the trust
//!   contract.
//! - **Name-backed** ([`AgentRuntimeId`], [`ProposerId`], [`EvaluatorId`],
//!   [`RendererId`], [`StopperId`], plus [`StageId::Custom`]) — human-meaningful strings used
//!   for stage attribution in budgets, events, and error records. Backed by
//!   `Cow<'static, str>` so `const`-known names are zero-allocation.

use std::borrow::Cow;
use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Defines a UUID-backed newtype identifier.
///
/// Each generated type implements `new()` (random v4 UUID), `from_uuid` /
/// `as_uuid` for explicit conversions, `Default` (delegates to `new`), and
/// `Display` (delegates to the underlying UUID). They are `Copy`, `Hash`,
/// `Ord`, and serde-transparent, so they round-trip as bare UUID strings.
macro_rules! uuid_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Generates a fresh random identifier.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Wraps an existing UUID without generating a new one.
            ///
            /// Use this when re-hydrating an identifier from storage; never
            /// when creating a new run-graph entry.
            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Returns the underlying UUID for interop with code that
            /// operates on raw UUIDs (storage, telemetry, external APIs).
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

uuid_id!(
    /// Identifier for one optimization run. Stamped on every event the run
    /// emits and every record it persists.
    RunId
);

uuid_id!(
    /// Graph-local identity of an artifact state inside a run.
    ///
    /// Distinct from [`ContentId`]. The same content can appear under
    /// multiple `CandidateId`s when different proposals reach the same
    /// artifact value — preserving causal history is the whole point of the
    /// distinction.
    CandidateId
);

uuid_id!(
    /// Identifier for one [proposal batch] — the sibling group of proposals
    /// produced by a single proposer call.
    ///
    /// [proposal batch]: https://docs.rs/leaven-core/latest/leaven_core/struct.ProposalBatch.html
    ProposalBatchId
);

uuid_id!(
    /// Identifier for one proposal record inside a batch.
    ///
    /// Stable across the proposal's lifetime: from creation through apply,
    /// success, or failure.
    ProposalId
);

uuid_id!(
    /// Identifier for an attempt to apply a proposal.
    ///
    /// One proposal may be applied at most once successfully but may be
    /// retried if the framework ever supports retry; each attempt gets its
    /// own ID so failures can be queried per-attempt rather than collapsed.
    ApplyAttemptId
);

uuid_id!(
    /// Identifier for one evaluation request issued through a [`RunContext`].
    ///
    /// Distinct from [`EvaluationSetId`]: the request identifies *who asked*
    /// and *when*; the evaluation set identifies *what to evaluate*.
    ///
    /// [`RunContext`]: https://docs.rs/leaven-engine/latest/leaven_engine/struct.RunContext.html
    EvaluationRequestId
);

uuid_id!(
    /// Identifier for an evaluation set expression registered with the run.
    ///
    /// Refers to the *unresolved* expression (e.g. `Recent { window: ... }`).
    /// For deterministic cache keys, callers want
    /// [`ResolvedEvaluationSetId`] instead.
    EvaluationSetId
);

uuid_id!(
    /// Identifier for an evaluation set after resolution against a concrete
    /// case-set version.
    ///
    /// This is what the cache keys against. Two evaluations of the same
    /// dynamic [`EvaluationSet::Recent`] expression issued at different
    /// iterations resolve to different `ResolvedEvaluationSetId`s and are
    /// not pooled.
    ///
    /// [`EvaluationSet::Recent`]: https://docs.rs/leaven-core/latest/leaven_core/enum.EvaluationSet.html
    ResolvedEvaluationSetId
);

uuid_id!(
    /// Identifier for one assessment record stored in the run graph.
    AssessmentId
);

uuid_id!(
    /// Identifier for one population/frontier instance inside a run.
    ///
    /// Optimizers may host multiple populations; events emitted by a
    /// population are tagged with this ID so consumers can attribute them.
    PopulationId
);

uuid_id!(
    /// Identifier for one render invocation.
    ///
    /// Used to correlate renderer events with the rendered output and to
    /// charge cost back to the originating render call.
    RenderId
);

uuid_id!(
    /// Identifier for one provider-agent runtime session.
    ///
    /// Agent sessions are outside the optimizer graph. This ID correlates the
    /// runtime transcript, workspace command records, and raw provider events
    /// emitted during one call to an agent runtime.
    AgentSessionId
);

uuid_id!(
    /// Identifier for one candidate run inside a reflective case.
    ///
    /// Agentic and LM reflective datasets can carry multiple attempts for one
    /// case. This ID names the run record independently from the dense case
    /// index so transcripts, checks, and produced outputs can be cited without
    /// overloading [`CaseId`].
    CaseRunId
);

uuid_id!(
    /// Identifier for one allocated workspace instance.
    WorkspaceId
);

uuid_id!(
    /// Identifier for one optimizer stage call.
    StageCallId
);

uuid_id!(
    /// Identifier for a durable receipt produced by one stage attempt.
    StageAttemptReceiptId
);

uuid_id!(
    /// Identifier for one query handled by a stage read authority.
    StageQueryId
);

uuid_id!(
    /// Identifier for one workspace entry written by setup or query.
    WorkspaceEntryId
);

uuid_id!(
    /// Identifier for one persisted run checkpoint.
    CheckpointId
);

uuid_id!(
    /// Monotonic-by-creation identifier for one engine iteration.
    ///
    /// The engine's run loop mints a fresh `IterationId` per outer step and
    /// stamps it on the iteration's events.
    IterationId
);

/// 32-byte content identity used as the evaluation cache's primary key.
///
/// `ContentId` is the deterministic fingerprint of an artifact state. The
/// cache trusts it absolutely: same `ContentId` means same observable content,
/// and two evaluations keyed on the same `ContentId` are deduplicated.
///
/// # Trust contract
///
/// The framework cannot enforce that an artifact's `ContentId` actually hashes
/// its observable state — that is the implementor's contract. Lying about it
/// silently produces incorrect cache results. Two safety mechanisms are
/// provided in higher crates:
///
/// - future `#[derive(Artifact)]` / `#[derive(ContentAddressed)]` macros once
///   a behavior-bearing derive crate exists
///   generate hash implementations that include every field by default,
///   skipping only those marked `#[content_skip]`.
/// - A dev-mode `verify_cache_consistency` flag re-evaluates on cache hits
///   and compares results, catching contract violations during testing.
///
/// Content-addressed external handles (git commit hashes, IPFS CIDs, docker
/// image digests) trivially satisfy the contract — the handle *is* the hash —
/// and can be used directly without further hashing.
///
/// # Width
///
/// 32 bytes (256 bits) is the recommended minimum for cross-machine, durable
/// caches. Truncated 64-bit hashes are unsafe at typical run scales (>10^5
/// candidates) due to birthday-paradox collisions; 128-bit non-cryptographic
/// hashes (xxh3-128) are acceptable for in-process caching only.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentId(pub [u8; 32]);

impl ContentId {
    /// Width in bytes.
    pub const BYTES: usize = 32;

    /// Hashes observable artifact bytes into a `ContentId` with BLAKE3.
    #[must_use]
    pub fn hash_bytes(bytes: impl AsRef<[u8]>) -> Self {
        Self(*blake3::hash(bytes.as_ref()).as_bytes())
    }

    /// Wraps a 32-byte hash into a `ContentId`.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the raw bytes for storage or comparison.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Returns the all-zero `ContentId`. Useful as a sentinel during testing
    /// or as a placeholder before a hash is computed; never use this as a
    /// real artifact identity.
    #[must_use]
    pub const fn zero() -> Self {
        Self([0; 32])
    }
}

impl fmt::Display for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cid:{}", hex::encode(&self.0[..8]))
    }
}

/// Index into a [`CaseSet`].
///
/// Cases are user-supplied evaluation inputs (tasks, prompts, repository
/// snapshots — whatever the run is evaluating against). The ID is just a
/// `u64` because case sets are typically dense, ordered, and built once per
/// run; UUIDs would burn space and ergonomics with no benefit.
///
/// [`CaseSet`]: https://docs.rs/leaven-core/latest/leaven_core/struct.CaseSet.html
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct CaseId(pub u64);

impl CaseId {
    /// Wraps a raw index.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Constructs a `CaseId` from a `usize` index.
    ///
    /// # Panics
    ///
    /// Panics if `index` does not fit in a `u64` (only possible on
    /// hypothetical >64-bit `usize` platforms — the conversion always
    /// succeeds on every realistic target).
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        Self(u64::try_from(index).expect("usize index fits in u64"))
    }
}

impl fmt::Display for CaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "case:{}", self.0)
    }
}

/// Defines a name-backed newtype identifier.
///
/// Each generated type wraps `Cow<'static, str>` so callers can mint a
/// const-known name without allocating (`new_const`) or build one from
/// runtime data (`new`, `From<String>`). They are `Hash`, `Ord`, and
/// serde-transparent.
macro_rules! name_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Cow<'static, str>);

        impl $name {
            /// Constructs the identifier from a `'static` string slice
            /// without allocating. Use this for compile-time-known names.
            #[must_use]
            pub const fn new_const(name: &'static str) -> Self {
                Self(Cow::Borrowed(name))
            }

            /// Constructs the identifier from an owned or borrowed string.
            ///
            /// Accepts anything convertible into `Cow<'static, str>`,
            /// including `&'static str` (zero allocation) and `String`
            /// (one allocation, takes ownership).
            #[must_use]
            pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
                Self(name.into())
            }

            /// Returns the underlying name as a string slice.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&'static str> for $name {
            fn from(s: &'static str) -> Self {
                Self::new_const(s)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(Cow::Owned(s))
            }
        }
    };
}

name_id!(
    /// Stable identity for an agent role inside a reflective dataset.
    ///
    /// This is deliberately name-backed rather than UUID-backed: values such
    /// as `worker`, `critic`, or `planner` must stay cache-stable across
    /// equivalent projections.
    AgentId
);

name_id!(
    /// Identifier for one provider-neutral agent runtime implementation.
    ///
    /// Runtime IDs name the execution substrate (`codex/app-server`,
    /// `claude-code/cli`, `fake`) without implying anything about proposals,
    /// assessments, candidates, or optimizer rhythm.
    AgentRuntimeId
);

name_id!(
    /// Identifier for one [`Proposer`] implementation.
    ///
    /// Conventionally namespaced (e.g. `gepa/reflective_mutation`,
    /// `meta_harness/claude_code`) so multiple proposers can coexist in a
    /// run without collision.
    ///
    /// [`Proposer`]: https://docs.rs/leaven-engine/latest/leaven_engine/trait.Proposer.html
    ProposerId
);

name_id!(
    /// Identifier for one [`Evaluator`] implementation.
    ///
    /// A simple run uses [`EvaluatorId::PRIMARY`]. Multi-evaluator runs
    /// (task scorer + pairwise judge + verifier) install distinct IDs so the
    /// optimizer can target a specific evaluator via `evaluate_with`.
    ///
    /// [`Evaluator`]: https://docs.rs/leaven-engine/latest/leaven_engine/trait.Evaluator.html
    EvaluatorId
);

name_id!(
    /// Identifier for one renderer implementation.
    ///
    /// Most renderers are stage-owned fields rather than registry entries;
    /// `RendererId` exists for the few cases where a renderer is shared
    /// across stages or installed for debug/inspection.
    RendererId
);

name_id!(
    /// Identifier for one stopper.
    ///
    /// Stoppers terminate the run loop; the ID is recorded on the
    /// resulting `StopReason` so consumers can attribute the stop.
    StopperId
);

impl EvaluatorId {
    /// Convention name for the run's primary task evaluator.
    ///
    /// Single-evaluator runs install under this ID so optimizers can find
    /// the evaluator without configuration. Multi-evaluator runs may still
    /// use it for the canonical task evaluator alongside specialized ones.
    pub const PRIMARY: Self = Self::new_const("primary");

    /// Convention name for a pairwise judge evaluator.
    ///
    /// Tournament and preference-learning optimizers use this when a run has
    /// both an ordinary task evaluator and a pairwise/listwise preference
    /// judge.
    pub const PAIRWISE_JUDGE: Self = Self::new_const("pairwise_judge");
}

/// Discriminated tag identifying which kind of stage is acting.
///
/// Every cost charge, error record, and budget snapshot is attributed to a
/// `StageId` so that retrospective analysis can answer questions like "how
/// much did the pairwise judge cost?" without text-matching against names.
///
/// `Custom` exists for stages that don't fit the four named categories
/// (e.g. an optimizer's bookkeeping pass that wants to spend cost without
/// being a proposer/evaluator/renderer/stopper).
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum StageId {
    /// A proposer stage (produces proposals).
    Proposer(ProposerId),
    /// An evaluator stage (produces assessments).
    Evaluator(EvaluatorId),
    /// A renderer stage (produces views or workspace content).
    Renderer(RendererId),
    /// A stopper stage (decides whether to terminate the run loop).
    Stopper(StopperId),
    /// An ad-hoc named stage for callers that don't fit the above shapes.
    Custom(Cow<'static, str>),
}

impl StageId {
    /// Wraps a [`ProposerId`] as a [`StageId`].
    #[must_use]
    pub fn from_proposer(id: ProposerId) -> Self {
        Self::Proposer(id)
    }

    /// Wraps an [`EvaluatorId`] as a [`StageId`].
    #[must_use]
    pub fn from_evaluator(id: EvaluatorId) -> Self {
        Self::Evaluator(id)
    }

    /// Wraps a [`RendererId`] as a [`StageId`].
    #[must_use]
    pub fn from_renderer(id: RendererId) -> Self {
        Self::Renderer(id)
    }

    /// Constructs a [`StageId::Custom`] from any name-like value.
    #[must_use]
    pub fn custom(name: impl Into<Cow<'static, str>>) -> Self {
        Self::Custom(name.into())
    }
}

impl fmt::Display for StageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Proposer(id) => write!(f, "proposer:{id}"),
            Self::Evaluator(id) => write!(f, "evaluator:{id}"),
            Self::Renderer(id) => write!(f, "renderer:{id}"),
            Self::Stopper(id) => write!(f, "stopper:{id}"),
            Self::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

impl Serialize for StageId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for StageId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse_wire(&raw).map_err(serde::de::Error::custom)
    }
}

impl StageId {
    fn parse_wire(raw: &str) -> Result<Self, String> {
        let (kind, name) = raw
            .split_once(':')
            .ok_or_else(|| format!("stage id `{raw}` is missing a kind prefix"))?;
        if name.is_empty() {
            return Err(format!("stage id `{raw}` has an empty stage name"));
        }
        Ok(match kind {
            "proposer" => Self::Proposer(ProposerId::from(name.to_owned())),
            "evaluator" => Self::Evaluator(EvaluatorId::from(name.to_owned())),
            "renderer" => Self::Renderer(RendererId::from(name.to_owned())),
            "stopper" => Self::Stopper(StopperId::from(name.to_owned())),
            "custom" => Self::Custom(Cow::Owned(name.to_owned())),
            _ => return Err(format!("stage id `{raw}` has unknown kind `{kind}`")),
        })
    }
}

/// Pointer to an out-of-graph blob.
///
/// Large operational artifacts — rendered prompts, raw model responses,
/// agent stdout — should not bloat the run graph. Stages stash them in a
/// blob store and put a `BlobRef` in [`MetadataValue::BlobRef`] so the graph
/// stays small while the data stays reachable.
///
/// [`MetadataValue::BlobRef`]: crate::metadata::MetadataValue::BlobRef
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct BlobRef {
    /// Identifier of the blob store backend (e.g. `"file"`, `"s3"`).
    pub store: String,
    /// Backend-specific key locating the blob.
    pub key: String,
}

/// Pointer to evidence held outside the run graph.
///
/// Some evidence shapes (multi-megabyte agent transcripts, full execution
/// traces) are too large to inline. The graph holds an `EvidenceRef`; the
/// actual bytes live in an `EvidenceStore` the engine was configured with.
/// Distinct from [`BlobRef`] so storage backends and access policies can
/// differ between operational blobs and run-evidence payloads.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// Identifier of the evidence store backend.
    pub store: String,
    /// Backend-specific key locating the evidence payload.
    pub key: String,
}

/// Pointer to a durable execution trace or transcript.
///
/// Reflection workspaces use `TraceRef` when a transcript is too large to
/// inline. It is separate from [`EvidenceRef`] because traces are operational
/// readback material first; callers can decide later whether to promote them
/// into semantic evidence.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct TraceRef {
    /// Identifier of the trace store backend.
    pub store: String,
    /// Backend-specific key locating the trace payload.
    pub key: String,
}
