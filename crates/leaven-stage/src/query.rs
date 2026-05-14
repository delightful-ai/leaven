use std::collections::BTreeSet;

use leaven_kernel::{AssessmentId, CandidateId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageQueryPolicy {
    pub allowed: AllowedQuerySet,
    pub prewarm: Vec<StageQuery>,
    pub max_queries: Option<usize>,
    pub max_materialized_bytes: Option<u64>,
}

impl StageQueryPolicy {
    #[must_use]
    pub fn minimal() -> Self {
        Self {
            allowed: AllowedQuerySet::none(),
            prewarm: Vec::new(),
            max_queries: Some(0),
            max_materialized_bytes: Some(0),
        }
    }

    #[must_use]
    pub fn bounded(
        allowed: AllowedQuerySet,
        prewarm: Vec<StageQuery>,
        max_queries: Option<usize>,
        max_materialized_bytes: Option<u64>,
    ) -> Self {
        Self {
            allowed,
            prewarm,
            max_queries,
            max_materialized_bytes,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AllowedQuerySet {
    allowed: BTreeSet<StageQueryKind>,
}

impl AllowedQuerySet {
    #[must_use]
    pub fn none() -> Self {
        Self {
            allowed: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn only(kinds: impl IntoIterator<Item = StageQueryKind>) -> Self {
        Self {
            allowed: kinds.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn reflection_default() -> Self {
        Self::only([
            StageQueryKind::Help,
            StageQueryKind::Candidate,
            StageQueryKind::Assessment,
            StageQueryKind::Lineage,
            StageQueryKind::Diff,
        ])
    }

    #[must_use]
    pub fn contains(&self, kind: StageQueryKind) -> bool {
        self.allowed.contains(&kind)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum StageQueryKind {
    Help,
    ListCandidates,
    Candidate,
    Assessment,
    Evidence,
    Lineage,
    Diff,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StageQuery {
    Help,
    ListCandidates,
    Candidate {
        id: CandidateId,
    },
    Assessment {
        id: AssessmentId,
    },
    Evidence,
    Lineage {
        candidate: CandidateId,
        depth: usize,
    },
    Diff {
        left: CandidateId,
        right: CandidateId,
    },
}

impl StageQuery {
    #[must_use]
    pub const fn kind(&self) -> StageQueryKind {
        match self {
            Self::Help => StageQueryKind::Help,
            Self::ListCandidates => StageQueryKind::ListCandidates,
            Self::Candidate { .. } => StageQueryKind::Candidate,
            Self::Assessment { .. } => StageQueryKind::Assessment,
            Self::Evidence => StageQueryKind::Evidence,
            Self::Lineage { .. } => StageQueryKind::Lineage,
            Self::Diff { .. } => StageQueryKind::Diff,
        }
    }
}
