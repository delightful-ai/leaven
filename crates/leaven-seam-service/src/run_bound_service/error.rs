use leaven_kernel::ProposalBatchId;
use thiserror::Error;

/// Errors from run-bound graph effect handling.
#[derive(Debug, Error)]
pub enum RunBoundGraphEffectError {
    /// The method is not a run-bound graph-write method.
    #[error("run-bound graph service cannot execute `{method}`")]
    UnsupportedMethod {
        /// Method name.
        method: String,
    },
    /// A required string field is missing.
    #[error("{field} must be a string")]
    MissingString {
        /// Field name.
        field: &'static str,
    },
    /// A required JSON value is missing.
    #[error("{field} must be present")]
    MissingValue {
        /// Field name.
        field: &'static str,
    },
    /// A graph-backed public-seam projection had an unexpected shape.
    #[error("graph-backed public-seam projection field `{field}` had invalid shape: {reason}")]
    InvalidProjection {
        /// Field path or semantic field name.
        field: &'static str,
        /// Expected shape or violated invariant.
        reason: &'static str,
    },
    /// The callback did not carry an apply proposal write.
    #[error("leaven/proposal.apply callback must carry an apply_proposal_batch write")]
    MissingApplyWrite,
    /// The callback did not carry a proposal submit write.
    #[error("leaven/proposal.submit_batch callback must carry a submit_proposal_batch write")]
    MissingProposalSubmitWrite,
    /// The callback did not carry an evaluation request write.
    #[error("leaven/evaluation.request callback must carry a request_evaluation write")]
    MissingEvaluationRequestWrite,
    /// The callback did not carry an assessment write.
    #[error("leaven/assessment.submit callback must carry a submit_assessments write")]
    MissingAssessmentWrite,
    /// The callback did not carry an event write.
    #[error("leaven/event.emit callback must carry an emit_run_event write")]
    MissingEventWrite,
    /// The event payload did not match the engine-owned external event payload.
    #[error("leaven/event.emit callback payload is not typed: {0}")]
    InvalidEventPayload(String),
    /// The host has no typed assessment lowerer installed.
    #[error("leaven/assessment.submit callback requires a typed host assessment lowerer")]
    MissingAssessmentSubmitter,
    /// The host has no typed proposal lowerer installed.
    #[error("leaven/proposal.submit_batch callback requires a typed host proposal lowerer")]
    MissingProposalSubmitter,
    /// The host has no typed evaluation request lowerer installed.
    #[error("leaven/evaluation.request callback requires a typed host evaluation request lowerer")]
    MissingEvaluationRequester,
    /// Host-side typed proposal lowering refused the payload.
    #[error("proposal submit payload refused by host lowerer: {0}")]
    ProposalSubmit(String),
    /// Host-side typed assessment lowering refused the payload.
    #[error("assessment submit payload refused by host lowerer: {0}")]
    AssessmentSubmit(String),
    /// Host-side typed evaluation request lowering refused the payload.
    #[error("evaluation request payload refused by host lowerer: {0}")]
    EvaluationRequest(String),
    /// A request was recorded but could not be read back from the graph.
    #[error("recorded evaluation request was not visible after RunContext mutation")]
    RecordedRequestMissing,
    /// The public proposal batch ref is malformed.
    #[error("proposal_batch must be a pb_<uuid> ref")]
    InvalidProposalBatchRef,
    /// The public evaluation request ref is malformed.
    #[error("evaluation_request_id must be an evalreq_<uuid> ref")]
    InvalidEvaluationRequestRef,
    /// The batch is not one of the batches registered with the service.
    #[error("proposal batch `{0}` is not registered with the run-bound graph service")]
    UnknownBatch(ProposalBatchId),
    /// `RunContext` rejected the write.
    #[error(transparent)]
    RunContext(#[from] leaven_engine::RunContextError),
    /// The graph-backed proposal report failed public-seam projection.
    #[error(transparent)]
    ProposalProjection(#[from] leaven_run::PublicProposalWriteReceiptProjectionError),
    /// The graph-backed assessment report failed public-seam projection.
    #[error(transparent)]
    AssessmentProjection(#[from] leaven_run::PublicAssessmentWriteReceiptProjectionError),
    /// The graph-backed evaluation request failed public-seam projection.
    #[error(transparent)]
    EvaluationProjection(#[from] leaven_run::PublicEvaluationJobProjectionError),
    /// The graph-backed extension result failed public-seam projection.
    #[error("graph-backed public-seam extension result projection failed: {0}")]
    ExtensionProjection(String),
    /// Canonical JSON hashing failed.
    #[error("failed to hash public seam receipt preimage: {0}")]
    Hash(String),
}
