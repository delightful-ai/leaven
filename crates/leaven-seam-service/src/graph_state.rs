use leaven_public_seam::{
    EvaluationJobDocument, PlanApplyProposalBatchOutcome, PlanApplyProposalBatchRequest,
    PlanEmitRunEventOutcome, PlanEmitRunEventRequest, PlanGraphQueryOutcome, PlanGraphReadScope,
    PlanSubmitAssessmentsOutcome, PlanSubmitAssessmentsRequest, PlanSubmitProposalBatchOutcome,
    PlanSubmitProposalBatchRequest, PublicSeamError,
};
use serde_json::{Value, json};

use crate::service::SeamGraphConfig;

#[derive(Clone, Debug)]
pub struct SeamGraphState {
    items: Vec<Value>,
    data_classes: Vec<String>,
    next_cursor: Option<String>,
    revision_index: u64,
}

impl SeamGraphState {
    pub(crate) fn new(config: &SeamGraphConfig) -> Self {
        Self {
            items: config.items.clone(),
            data_classes: config.data_classes.clone(),
            next_cursor: config.next_cursor.clone(),
            revision_index: 0,
        }
    }

    pub(crate) fn query(
        &self,
        scope: PlanGraphReadScope<'_>,
        default_revision: &str,
    ) -> PlanGraphQueryOutcome {
        let graph_revision = graph_query_revision(scope, default_revision);
        let mut outcome = PlanGraphQueryOutcome::new(self.items.clone(), graph_revision)
            .with_data_classes(self.data_classes.clone());
        if let Some(next_cursor) = &self.next_cursor {
            outcome = outcome.with_next_cursor(next_cursor.clone());
        }
        outcome
    }

    pub(crate) fn emit_run_event(
        &mut self,
        request: PlanEmitRunEventRequest<'_>,
    ) -> PlanEmitRunEventOutcome {
        let event_id = format!("event_{}", request.name());
        let revision = self.next_revision(request.base_revision(), "event_emit");
        self.items.push(json!({
            "kind": "event_summary",
            "event_kind": request.write().event_kind(),
            "revision": revision,
            "payload": {
                "kind": "event_emitted",
                "event_id": event_id,
                "event_kind": request.write().event_kind(),
                "payload_schema": request.write().payload_schema(),
                "value": request.write().payload(),
                "visibility": request.write().visibility()
            }
        }));
        PlanEmitRunEventOutcome::new(event_id, revision)
    }

    pub(crate) fn submit_proposal_batch(
        &mut self,
        request: &PlanSubmitProposalBatchRequest<'_>,
    ) -> Result<PlanSubmitProposalBatchOutcome, PublicSeamError> {
        let proposal_ids = (0..request.proposal_count()?)
            .map(|index| format!("prop_{}_{}", request.name(), index))
            .collect::<Vec<_>>();
        let batch_id = format!("pb_{}", request.name());
        let revision = self.next_revision(request.base_revision(), "proposal_submit");
        self.items.push(json!({
            "kind": "event_summary",
            "event_kind": "proposal.submit_batch",
            "revision": revision,
            "payload": {
                "kind": "proposal_batch_submitted",
                "proposal_batch": batch_id,
                "proposal_ids": proposal_ids
            }
        }));
        Ok(PlanSubmitProposalBatchOutcome::new(
            batch_id,
            proposal_ids,
            revision,
        ))
    }

    pub(crate) fn apply_proposal_batch(
        &mut self,
        request: &PlanApplyProposalBatchRequest<'_>,
    ) -> Result<PlanApplyProposalBatchOutcome, PublicSeamError> {
        let batch = request
            .write()
            .get("proposal_batch")
            .and_then(Value::as_str)
            .ok_or_else(|| PublicSeamError::InvalidPlan {
                message: "apply_proposal_batch must carry proposal_batch".to_owned(),
            })?;
        let candidates = vec![format!("cand_{}_applied", sanitize_id_fragment(batch))];
        let revision = self.next_revision(request.base_revision(), "proposal_apply");
        self.items.push(json!({
            "kind": "event_summary",
            "event_kind": "proposal.apply",
            "revision": revision,
            "payload": {
                "kind": "proposal_batch_applied",
                "proposal_batch": batch,
                "created_candidates": candidates
            }
        }));
        Ok(PlanApplyProposalBatchOutcome::new(candidates, revision))
    }

    pub(crate) fn submit_assessments(
        &mut self,
        request: &PlanSubmitAssessmentsRequest<'_>,
    ) -> Result<PlanSubmitAssessmentsOutcome, PublicSeamError> {
        let assessment_ids = (0..request.assessment_count()?)
            .map(|index| format!("assess_{}_{}", request.name(), index))
            .collect::<Vec<_>>();
        let evaluation_request_id = request.evaluation_request_id()?.to_owned();
        let revision = self.next_revision(request.base_revision(), "assessment_submit");
        self.items.push(json!({
            "kind": "event_summary",
            "event_kind": "assessment.submit",
            "revision": revision,
            "payload": {
                "kind": "assessments_submitted",
                "evaluation_request_id": evaluation_request_id,
                "assessment_ids": assessment_ids
            }
        }));
        Ok(PlanSubmitAssessmentsOutcome::new(assessment_ids, revision))
    }

    pub(crate) fn record_evaluation_request(
        &mut self,
        name: &str,
        job: &EvaluationJobDocument,
        base_revision: &str,
    ) -> String {
        let revision = self.next_revision(base_revision, "evaluation_request");
        self.items.push(json!({
            "kind": "event_summary",
            "event_kind": "evaluation.request",
            "revision": revision,
            "payload": {
                "kind": "evaluation_requested",
                "name": name,
                "evaluation_request_id": job.request_id(),
                "evaluator_id": job.evaluator_id()
            }
        }));
        revision
    }

    fn next_revision(&mut self, base_revision: &str, suffix: &str) -> String {
        self.revision_index += 1;
        format!("{base_revision}_{suffix}_{}", self.revision_index)
    }
}

fn graph_query_revision(scope: PlanGraphReadScope<'_>, default_revision: &str) -> String {
    match scope {
        PlanGraphReadScope::LatestAtStart { revision }
        | PlanGraphReadScope::AtRevision { revision } => revision.to_owned(),
        PlanGraphReadScope::SinceRevision { since: _, until } => {
            until.unwrap_or(default_revision).to_owned()
        }
    }
}

fn sanitize_id_fragment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
