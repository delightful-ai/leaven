//! Run-bound graph-write service for public-seam worker callbacks.
//!
//! This module is the durable-service counterpart to the configured
//! `SeamTextProblem` proof path. It does not own transport, graph internals, or
//! optimizer strategy. A run/stage owner binds a live [`RunContext`] and typed
//! lowerers, then this service routes locked public-seam method bodies through
//! the engine finalizers and projects receipt-bound extension results.

use std::cell::RefCell;
use std::collections::BTreeMap;

use leaven_core::{Assessment, EvaluationRequest, OptimizationProblem};
use leaven_engine::{ProposalBatchReport, RunContext, RunEvent};
use leaven_kernel::{EvaluatorId, Fingerprint, Metered, ProposalBatchId};
use leaven_seam_runtime::{SeamPlanRequest, SeamService, SeamServiceError, SeamStageRunRequest};
use serde_json::Value;

mod error;
mod extension_result;
mod params;

pub use error::RunBoundGraphEffectError;

use extension_result::{
    EventEmitExtensionContext, assessment_submit_extension_result,
    evaluation_request_extension_result, event_emit_extension_result,
    proposal_apply_extension_result,
};
use params::{
    evaluation_request_id, event_emit_write, proposal_batch_id, request_evaluation_write,
};

type AssessmentSubmitter<'service, P> =
    dyn Fn(&Value) -> Result<Metered<Vec<Assessment<P>>>, String> + 'service;
type EvaluationRequester<'service> =
    dyn Fn(&Value) -> Result<RunBoundEvaluationRequest, String> + 'service;

/// Typed evaluation request produced by a host-owned public payload lowerer.
pub struct RunBoundEvaluationRequest {
    /// Evaluator identity to record on the engine evaluation request.
    pub evaluator: EvaluatorId,
    /// Runtime fingerprint used for public job identity.
    pub evaluator_fingerprint: Fingerprint,
    /// Concrete engine request for this run.
    pub request: EvaluationRequest,
}

/// Run-bound service for worker-initiated public-seam graph writes.
pub struct RunBoundGraphEffectService<'service, 'run, P: OptimizationProblem> {
    context: RefCell<&'service mut RunContext<'run, P>>,
    batches: BTreeMap<ProposalBatchId, ProposalBatchReport>,
    assessment_submitter: Option<Box<AssessmentSubmitter<'service, P>>>,
    evaluation_requester: Option<Box<EvaluationRequester<'service>>>,
    capability_fingerprint: String,
    policy_fingerprint: String,
    base_revision: String,
    final_revision: String,
    started_at: String,
    completed_at: String,
}

impl<'service, 'run, P: OptimizationProblem> RunBoundGraphEffectService<'service, 'run, P> {
    /// Binds a live run context and the proposal batches workers may apply.
    pub fn new(
        context: &'service mut RunContext<'run, P>,
        batches: impl IntoIterator<Item = ProposalBatchReport>,
        capability_fingerprint: impl Into<String>,
        policy_fingerprint: impl Into<String>,
        base_revision: impl Into<String>,
        final_revision: impl Into<String>,
    ) -> Self {
        Self {
            context: RefCell::new(context),
            batches: batches
                .into_iter()
                .map(|batch| (batch.batch_id, batch))
                .collect(),
            assessment_submitter: None,
            evaluation_requester: None,
            capability_fingerprint: capability_fingerprint.into(),
            policy_fingerprint: policy_fingerprint.into(),
            base_revision: base_revision.into(),
            final_revision: final_revision.into(),
            started_at: "2026-06-04T00:00:00Z".to_owned(),
            completed_at: "2026-06-04T00:00:01Z".to_owned(),
        }
    }

    /// Overrides deterministic receipt timestamps for tests or an owning server.
    #[must_use]
    pub fn with_receipt_timing(
        mut self,
        started_at: impl Into<String>,
        completed_at: impl Into<String>,
    ) -> Self {
        self.started_at = started_at.into();
        self.completed_at = completed_at.into();
        self
    }

    /// Installs host-side lowering for assessment payloads.
    #[must_use]
    pub fn with_assessment_submitter(
        mut self,
        submitter: impl Fn(&Value) -> Result<Metered<Vec<Assessment<P>>>, String> + 'service,
    ) -> Self {
        self.assessment_submitter = Some(Box::new(submitter));
        self
    }

    /// Installs host-side lowering for evaluation request payloads.
    #[must_use]
    pub fn with_evaluation_requester(
        mut self,
        requester: impl Fn(&Value) -> Result<RunBoundEvaluationRequest, String> + 'service,
    ) -> Self {
        self.evaluation_requester = Some(Box::new(requester));
        self
    }

    /// Executes one locked graph-write method against the bound run context.
    pub fn handle_method(
        &self,
        method: &str,
        params: &Value,
    ) -> Result<Value, RunBoundGraphEffectError> {
        match method {
            "leaven/proposal.apply" => self.proposal_apply(params),
            "leaven/evaluation.request" => self.evaluation_request(params),
            "leaven/assessment.submit" => self.assessment_submit(params),
            "leaven/event.emit" => self.event_emit(params),
            other => Err(RunBoundGraphEffectError::UnsupportedMethod {
                method: other.to_owned(),
            }),
        }
    }

    fn proposal_apply(&self, params: &Value) -> Result<Value, RunBoundGraphEffectError> {
        let plan_id = params::string_field(params, "plan_id")?;
        let batch_id = proposal_batch_id(params)?;
        let batch = self
            .batches
            .get(&batch_id)
            .ok_or(RunBoundGraphEffectError::UnknownBatch(batch_id))?
            .clone();
        let mut context = self.context.borrow_mut();
        let apply = context.apply_batch(batch_id)?;
        let graph = context.graph();
        let plan_result = leaven_run::PublicProposalWriteReceiptContext::new(
            plan_id,
            &self.base_revision,
            &self.final_revision,
            &self.capability_fingerprint,
            &self.policy_fingerprint,
        )
        .with_submit_timing(&self.started_at, &self.started_at)
        .with_apply_timing(&self.started_at, &self.completed_at)
        .proposal_apply_plan_result(&graph, &batch, &apply)?;
        proposal_apply_extension_result(&plan_result)
    }

    fn evaluation_request(&self, params: &Value) -> Result<Value, RunBoundGraphEffectError> {
        request_evaluation_write(params)?;
        let requester = self
            .evaluation_requester
            .as_ref()
            .ok_or(RunBoundGraphEffectError::MissingEvaluationRequester)?;
        let request = requester(params).map_err(RunBoundGraphEffectError::EvaluationRequest)?;
        let mut context = self.context.borrow_mut();
        let request_id = context.request_evaluation(
            request.evaluator,
            request.evaluator_fingerprint,
            request.request,
        )?;
        let graph = context.graph();
        let request = graph
            .evaluation_request(request_id)
            .ok_or(RunBoundGraphEffectError::RecordedRequestMissing)?;
        let job_context = leaven_run::PublicEvaluationJobContext::new(
            "sc_run_bound_evaluation_request",
            &self.base_revision,
            &self.capability_fingerprint,
            &self.policy_fingerprint,
            "2026-06-04T00:10:00Z",
        )
        .with_evaluation_request_receipt_timing(&self.started_at, &self.completed_at);
        let job = job_context.evaluation_job_document(&graph, &request)?;
        let plan_result = job_context.evaluation_request_receipt_plan_result(&job)?;
        evaluation_request_extension_result(&plan_result)
    }

    fn assessment_submit(&self, params: &Value) -> Result<Value, RunBoundGraphEffectError> {
        let plan_id = params::string_field(params, "plan_id")?;
        let request_id = evaluation_request_id(params)?;
        let submitter = self
            .assessment_submitter
            .as_ref()
            .ok_or(RunBoundGraphEffectError::MissingAssessmentSubmitter)?;
        let assessments = submitter(params).map_err(RunBoundGraphEffectError::AssessmentSubmit)?;
        let mut context = self.context.borrow_mut();
        let report = context.submit_assessments(request_id, assessments)?;
        let graph = context.graph();
        let plan_result = leaven_run::PublicAssessmentWriteReceiptContext::new(
            plan_id,
            &self.base_revision,
            &self.final_revision,
            &self.capability_fingerprint,
            &self.policy_fingerprint,
        )
        .with_timing(&self.started_at, &self.completed_at)
        .submit_assessments_plan_result(&graph, &report)?;
        assessment_submit_extension_result(&plan_result)
    }

    fn event_emit(&self, params: &Value) -> Result<Value, RunBoundGraphEffectError> {
        let plan_id = params::string_field(params, "plan_id")?;
        let event = event_emit_write(params)?;
        let event_id = format!("event_{}", event.name);
        self.context
            .borrow_mut()
            .emit(RunEvent::ExternalEventEmitted {
                event_id: event_id.clone(),
                event_kind: event.event_kind.to_owned(),
                payload_schema: event.payload_schema.to_owned(),
                payload: event.payload.clone(),
                visibility: event.visibility.to_owned(),
            });
        event_emit_extension_result(
            EventEmitExtensionContext {
                plan_id,
                name: event.name,
                write: event.write,
                event_id: &event_id,
                base_revision: &self.base_revision,
                final_revision: &self.final_revision,
                capability_fingerprint: &self.capability_fingerprint,
                policy_fingerprint: &self.policy_fingerprint,
                started_at: &self.started_at,
                completed_at: &self.completed_at,
            },
            params,
        )
    }
}

impl<P: OptimizationProblem> SeamService for RunBoundGraphEffectService<'_, '_, P> {
    fn handle_plan(&self, request: SeamPlanRequest<'_>) -> Result<Value, SeamServiceError> {
        self.handle_method(request.method().as_str(), request.params())
            .map_err(|error| SeamServiceError::execution(error.to_string()))
    }

    fn handle_stage_run(
        &self,
        _request: SeamStageRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        Err(SeamServiceError::unavailable("leaven/stage.run"))
    }
}

#[cfg(test)]
mod tests;
