use serde_json::Value;

use crate::PublicSeamError;

use super::super::{invalid_plan, required_string};

/// Lowered `submit_assessments` write passed to a plan execution host.
#[derive(Clone, Debug)]
pub struct PlanSubmitAssessmentsRequest<'a> {
    pub(super) name: &'a str,
    pub(super) write: &'a Value,
    pub(super) base_revision: &'a str,
}

impl<'a> PlanSubmitAssessmentsRequest<'a> {
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

    /// Evaluation request id carried by the write.
    pub fn evaluation_request_id(&self) -> Result<&str, PublicSeamError> {
        required_string(
            self.write.get("evaluation_request_id"),
            "evaluation_request_id",
        )
    }

    /// Number of assessments submitted by this write.
    pub fn assessment_count(&self) -> Result<usize, PublicSeamError> {
        self.write
            .get("assessments")
            .and_then(Value::as_array)
            .map(Vec::len)
            .ok_or_else(|| invalid_plan("submit_assessments must carry assessments"))
    }
}

/// Host outcome for a typed `submit_assessments` write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanSubmitAssessmentsOutcome {
    pub(super) assessment_ids: Vec<String>,
    pub(super) committed_revision: String,
    pub(super) data_classes: Vec<String>,
    pub(super) replayability: String,
}

impl PlanSubmitAssessmentsOutcome {
    /// Creates an assessment-batch receipt outcome.
    pub fn new(assessment_ids: Vec<String>, committed_revision: impl Into<String>) -> Self {
        Self {
            assessment_ids,
            committed_revision: committed_revision.into(),
            data_classes: vec!["public".to_owned()],
            replayability: "fully_managed".to_owned(),
        }
    }

    pub(crate) fn assessment_ids(&self) -> &[String] {
        &self.assessment_ids
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
