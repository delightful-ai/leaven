//! Identifier newtypes.
//!
//! Every long-lived record in the run graph has a typed ID. UUIDs are
//! used for graph-local identities (candidates, proposals, attempts,
//! requests) so they can be allocated without coordination. Stage,
//! proposer, evaluator, and renderer identities are name-based because
//! they are configured before a run starts and frequently appear in
//! literals, logs, and configs.

use std::borrow::Cow;
use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! uuid_id {
    (
        $(#[$meta:meta])*
        $name:ident
    ) => {
        $(#[$meta])*
        #[derive(
            Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            #[must_use]
            pub const fn as_uuid(&self) -> Uuid {
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
    /// One optimization run.
    RunId
);
uuid_id!(
    /// Graph-local occurrence of an artifact in a run.
    ///
    /// Distinct from [`crate::artifact::ContentId`]: same content can
    /// appear in multiple candidates via different proposals, and the
    /// causal history that produced each occurrence is preserved.
    CandidateId
);
uuid_id!(
    /// A batch of proposals produced from a single proposer call.
    ProposalBatchId
);
uuid_id!(
    /// A single proposal record.
    ProposalId
);
uuid_id!(
    /// One attempt to apply a proposal. Both successes and failures
    /// produce attempt records.
    ApplyAttemptId
);
uuid_id!(EvaluationRequestId);
uuid_id!(AssessmentId);
uuid_id!(PopulationId);

/// Monotonic iteration counter inside a run.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct IterationId(pub u64);

/// String-typed identity for a configurable stage component (proposer,
/// evaluator, renderer). Configured at run setup; used in events, logs,
/// trust policies, and cache keys.
///
/// `Cow<'static, str>` lets implementations declare a `const` ID
/// without runtime allocation while still allowing dynamic IDs from
/// configuration files.
macro_rules! name_id {
    (
        $(#[$meta:meta])*
        $name:ident
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Cow<'static, str>);

        impl $name {
            #[must_use]
            pub const fn new_const(name: &'static str) -> Self {
                Self(Cow::Borrowed(name))
            }

            #[must_use]
            pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
                Self(name.into())
            }

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
    /// Identity of a configured proposer.
    ProposerId
);
name_id!(
    /// Identity of a configured evaluator.
    EvaluatorId
);
name_id!(
    /// Identity of a configured renderer.
    RendererId
);

impl EvaluatorId {
    /// The default evaluator if exactly one is configured.
    pub const PRIMARY: Self = Self::new_const("primary");
}

/// Identity of a stage invocation: where in the run-time topology a
/// cost was charged or an error was attributed.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum StageId {
    Proposer(ProposerId),
    Evaluator(EvaluatorId),
    Renderer(RendererId),
    /// Optimizer-internal step that does not correspond to any of the
    /// configured stage components (e.g. population maintenance, custom
    /// optimizer work).
    Custom(Cow<'static, str>),
}

impl StageId {
    #[must_use]
    pub fn from_proposer(id: ProposerId) -> Self {
        Self::Proposer(id)
    }

    #[must_use]
    pub fn from_evaluator(id: EvaluatorId) -> Self {
        Self::Evaluator(id)
    }

    #[must_use]
    pub fn from_renderer(id: RendererId) -> Self {
        Self::Renderer(id)
    }

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
            Self::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

/// Stable identity of a case-set partition (e.g. `SEARCH`, `TEST`,
/// `HOLDOUT`). Used in trust policies, frontier filters, and
/// evaluation set construction.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PartitionId(pub Cow<'static, str>);

impl PartitionId {
    #[must_use]
    pub const fn new_const(name: &'static str) -> Self {
        Self(Cow::Borrowed(name))
    }

    #[must_use]
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self(name.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PartitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
