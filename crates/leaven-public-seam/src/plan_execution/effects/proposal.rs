use serde_json::Value;

use crate::PublicSeamError;

use super::super::{invalid_plan, required_string};

/// Lowered `submit_proposal_batch` or `apply_proposal_batch` write passed to a plan execution host.
#[derive(Clone, Debug)]
pub struct PlanProposalWriteRequest<'a> {
    pub(super) name: &'a str,
    pub(super) write: &'a Value,
    pub(super) base_revision: &'a str,
}

impl<'a> PlanProposalWriteRequest<'a> {
    pub(crate) const fn new(name: &'a str, write: &'a Value, base_revision: &'a str) -> Self {
        Self {
            name,
            write,
            base_revision,
        }
    }

    /// Operation variable name for the write.
    pub const fn name(&self) -> &str {
        self.name
    }

    /// Base graph revision used by this write.
    pub const fn base_revision(&self) -> &str {
        self.base_revision
    }

    /// Typed write body.
    pub const fn write(&self) -> &Value {
        self.write
    }

    /// Number of proposals submitted by this batch.
    pub fn proposal_count(&self) -> Result<usize, PublicSeamError> {
        self.write
            .get("proposals")
            .and_then(Value::as_array)
            .map(Vec::len)
            .ok_or_else(|| invalid_plan("submit_proposal_batch must carry proposals"))
    }

    /// Proposal-batch id carried by an `apply_proposal_batch` write.
    pub fn proposal_batch(&self) -> Result<&str, PublicSeamError> {
        required_string(self.write.get("proposal_batch"), "proposal_batch")
    }
}

/// Lowered `submit_proposal_batch` write passed to a plan execution host.
pub type PlanSubmitProposalBatchRequest<'a> = PlanProposalWriteRequest<'a>;

/// Lowered `apply_proposal_batch` write passed to a plan execution host.
pub type PlanApplyProposalBatchRequest<'a> = PlanProposalWriteRequest<'a>;

/// Host outcome for a typed `submit_proposal_batch` write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanSubmitProposalBatchOutcome {
    pub(super) batch_id: String,
    pub(super) proposal_ids: Vec<String>,
    pub(super) committed_revision: String,
    pub(super) data_classes: Vec<String>,
    pub(super) replayability: String,
}

impl PlanSubmitProposalBatchOutcome {
    /// Creates a proposal-batch receipt outcome.
    pub fn new(
        batch_id: impl Into<String>,
        proposal_ids: Vec<String>,
        committed_revision: impl Into<String>,
    ) -> Self {
        Self {
            batch_id: batch_id.into(),
            proposal_ids,
            committed_revision: committed_revision.into(),
            data_classes: vec!["public".to_owned()],
            replayability: "fully_managed".to_owned(),
        }
    }

    pub(crate) fn batch_id(&self) -> &str {
        &self.batch_id
    }

    pub(crate) fn proposal_ids(&self) -> &[String] {
        &self.proposal_ids
    }

    pub(crate) fn committed_revision(&self) -> &str {
        &self.committed_revision
    }

    pub(crate) fn data_classes(&self) -> &[String] {
        &self.data_classes
    }

    pub(crate) fn replayability(&self) -> &str {
        &self.replayability
    }
}

/// Host outcome for a typed `apply_proposal_batch` write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanApplyProposalBatchOutcome {
    pub(super) created_candidates: Vec<String>,
    pub(super) committed_revision: String,
    pub(super) data_classes: Vec<String>,
    pub(super) replayability: String,
}

impl PlanApplyProposalBatchOutcome {
    /// Creates an apply receipt outcome.
    pub fn new(created_candidates: Vec<String>, committed_revision: impl Into<String>) -> Self {
        Self {
            created_candidates,
            committed_revision: committed_revision.into(),
            data_classes: vec!["public".to_owned()],
            replayability: "fully_managed".to_owned(),
        }
    }

    pub(crate) fn created_candidates(&self) -> &[String] {
        &self.created_candidates
    }

    pub(crate) fn committed_revision(&self) -> &str {
        &self.committed_revision
    }

    pub(crate) fn data_classes(&self) -> &[String] {
        &self.data_classes
    }

    pub(crate) fn replayability(&self) -> &str {
        &self.replayability
    }
}
