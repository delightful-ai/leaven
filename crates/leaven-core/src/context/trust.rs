//! Trust policy: what each actor in a run is allowed to read.
//!
//! `TrustPolicy` is configured at run setup. It produces actor-specific
//! `ReadScope`s that filter graph views, evidence queries, and
//! evaluation requests. The framework cannot stop a fully-trusted
//! optimizer from threading hidden data through a custom proposer's
//! request type — the trust layer makes the *correct* boundary
//! cheap and *violations* visible.
//!
//! Most types are sketched here. Detailed policy methods land with the
//! engine.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::PartitionId;

/// Per-actor read filter applied to graph views and evidence queries.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReadScope {
    pub visible_partitions: BTreeSet<PartitionId>,
    pub visible_evidence: EvidenceVisibility,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum EvidenceVisibility {
    #[default]
    Full,
    ScoresOnly,
    SummariesOnly,
    None,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrustPolicy {
    pub hidden_from_proposers: Vec<PartitionId>,
    pub hidden_from_optimizers: Vec<PartitionId>,
    pub hidden_from_callbacks: Vec<PartitionId>,
}

#[derive(Debug, Error)]
pub enum TrustViolation {
    #[error("actor `{actor}` is not allowed to evaluate against partition `{partition}`")]
    PartitionForbidden {
        actor: String,
        partition: PartitionId,
    },

    #[error("actor `{actor}` is not allowed to read evidence at the requested visibility")]
    EvidenceVisibility { actor: String },
}
