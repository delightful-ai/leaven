use std::fmt::Write as _;

use leaven_core::{
    AssessmentGranularity, EvaluationPurpose, EvaluationRequest, OptimizationProblem,
};
use leaven_engine::{EvaluationRequestView, RunGraphView};
use leaven_kernel::{
    CandidateId, CaseId, EvaluationRequestId, Fingerprint, ResolvedEvaluationSetId, RunId,
};
use serde_json::{Map, Value, json};
use thiserror::Error;

/// Public-seam fields supplied by the external-worker dispatch layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicEvaluationJobContext {
    stage_call_id: String,
    base_revision: String,
    capability_fingerprint: String,
    deadline_at: String,
    evaluation_attempt_id: Option<String>,
    target_egress_policy_ref: Option<String>,
}

impl PublicEvaluationJobContext {
    /// Creates dispatch-layer identity for a public-seam evaluation job.
    #[must_use]
    pub fn new(
        stage_call_id: impl Into<String>,
        base_revision: impl Into<String>,
        capability_fingerprint: impl Into<String>,
        deadline_at: impl Into<String>,
    ) -> Self {
        Self {
            stage_call_id: stage_call_id.into(),
            base_revision: base_revision.into(),
            capability_fingerprint: capability_fingerprint.into(),
            deadline_at: deadline_at.into(),
            evaluation_attempt_id: None,
            target_egress_policy_ref: None,
        }
    }

    /// Adds a public evaluation-attempt id.
    #[must_use]
    pub fn with_evaluation_attempt_id(mut self, id: impl Into<String>) -> Self {
        self.evaluation_attempt_id = Some(id.into());
        self
    }

    /// Adds a target-egress policy fingerprint for evaluator target reads.
    #[must_use]
    pub fn with_target_egress_policy_ref(mut self, policy: impl Into<String>) -> Self {
        self.target_egress_policy_ref = Some(policy.into());
        self
    }

    /// Projects a recorded engine evaluation request into the locked public-seam job document.
    ///
    /// The returned value still needs validation by `leaven-public-seam`; this
    /// helper only lowers runtime-owned identity into the active wire shape.
    pub fn evaluation_job_document<P>(
        &self,
        graph: &RunGraphView<'_, P>,
        request: &EvaluationRequestView<'_>,
    ) -> Result<Value, PublicEvaluationJobProjectionError>
    where
        P: OptimizationProblem,
    {
        let mut document = Map::from_iter([
            (
                "schema_version".to_owned(),
                json!("leaven.evaluation_job.v1"),
            ),
            ("run".to_owned(), json!(run_ref(graph.run_id()))),
            ("stage_call_id".to_owned(), json!(self.stage_call_id)),
            (
                "evaluation_request_id".to_owned(),
                json!(evaluation_request_ref(request.id())),
            ),
            ("base_revision".to_owned(), json!(self.base_revision)),
            ("deadline_at".to_owned(), json!(self.deadline_at)),
            ("kind".to_owned(), kind_value(request.request())),
            (
                "resolved_set".to_owned(),
                resolved_set_value(request.resolved_set()),
            ),
            (
                "granularity".to_owned(),
                json!(granularity_value(request.request())?),
            ),
            (
                "purpose".to_owned(),
                json!(purpose_value(request.request())),
            ),
            (
                "capability_fingerprint".to_owned(),
                json!(self.capability_fingerprint),
            ),
            (
                "evaluator_id".to_owned(),
                json!(request.evaluator().as_str()),
            ),
            (
                "evaluator_fingerprint".to_owned(),
                json!(runtime_fingerprint(request.evaluator_fingerprint())),
            ),
        ]);
        if let Some(id) = &self.evaluation_attempt_id {
            document.insert("evaluation_attempt_id".to_owned(), json!(id));
        }
        if let Some(policy) = &self.target_egress_policy_ref {
            document.insert("target_egress_policy_ref".to_owned(), json!(policy));
        }
        Ok(Value::Object(document))
    }
}

/// Errors raised while projecting runtime evaluation identity into the V1 wire shape.
#[derive(Debug, Error)]
pub enum PublicEvaluationJobProjectionError {
    /// The public evaluation-job schema has no representation for this engine granularity.
    #[error("public-seam evaluation jobs do not support `{granularity}` granularity")]
    UnsupportedGranularity {
        /// Engine granularity name.
        granularity: &'static str,
    },
}

fn kind_value(request: &EvaluationRequest) -> Value {
    match request {
        EvaluationRequest::Independent { candidates, .. } => json!({
            "kind": "independent",
            "candidates": candidates.iter().copied().map(candidate_ref).collect::<Vec<_>>()
        }),
        EvaluationRequest::Pairwise { left, right, .. } => json!({
            "kind": "pairwise",
            "pairs": [{
                "left": candidate_ref(*left),
                "right": candidate_ref(*right)
            }]
        }),
        EvaluationRequest::Listwise { candidates, .. } => json!({
            "kind": "listwise",
            "candidates": candidates.iter().copied().map(candidate_ref).collect::<Vec<_>>()
        }),
    }
}

fn resolved_set_value(set: &leaven_core::ResolvedEvaluationSet) -> Value {
    let case_ids = set
        .case_ids
        .iter()
        .copied()
        .map(case_ref)
        .collect::<Vec<_>>();
    json!({
        "id": resolved_set_ref(set.id),
        "case_ids": case_ids,
        "case_count": set.case_ids.len(),
        "case_set_version": set.case_set_version.0,
        "partition_summary": {
            "resolved": set.case_ids.len()
        }
    })
}

fn granularity_value(
    request: &EvaluationRequest,
) -> Result<&'static str, PublicEvaluationJobProjectionError> {
    let granularity = match request {
        EvaluationRequest::Independent { granularity, .. }
        | EvaluationRequest::Pairwise { granularity, .. }
        | EvaluationRequest::Listwise { granularity, .. } => granularity,
    };
    match granularity {
        AssessmentGranularity::Aggregate => Ok("aggregate"),
        AssessmentGranularity::PerCase => Ok("per_case"),
        AssessmentGranularity::Both => {
            Err(PublicEvaluationJobProjectionError::UnsupportedGranularity {
                granularity: "both",
            })
        }
    }
}

fn purpose_value(request: &EvaluationRequest) -> &'static str {
    let purpose = match request {
        EvaluationRequest::Independent { purpose, .. }
        | EvaluationRequest::Pairwise { purpose, .. }
        | EvaluationRequest::Listwise { purpose, .. } => purpose,
    };
    match purpose {
        EvaluationPurpose::SeedBaseline
        | EvaluationPurpose::Feedback
        | EvaluationPurpose::Screening
        | EvaluationPurpose::Search => "train",
        EvaluationPurpose::Validation | EvaluationPurpose::Selection => "validation",
        EvaluationPurpose::FinalTest => "test",
        EvaluationPurpose::Probe => "diagnostic",
        EvaluationPurpose::Custom(_) => "custom",
    }
}

fn run_ref(id: RunId) -> String {
    uuid_ref("run", id.as_uuid())
}

fn candidate_ref(id: CandidateId) -> String {
    uuid_ref("cand", id.as_uuid())
}

fn evaluation_request_ref(id: EvaluationRequestId) -> String {
    uuid_ref("evalreq", id.as_uuid())
}

fn resolved_set_ref(id: ResolvedEvaluationSetId) -> String {
    uuid_ref("rset", id.as_uuid())
}

fn case_ref(id: CaseId) -> String {
    format!("case_{}", id.0)
}

fn uuid_ref(prefix: &str, id: uuid::Uuid) -> String {
    format!("{prefix}_{id}")
}

fn runtime_fingerprint(fingerprint: Fingerprint) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in fingerprint.0 {
        write!(&mut encoded, "{byte:02x}").expect("writing to string cannot fail");
    }
    format!("fp_runtime_blake3_{encoded}")
}
