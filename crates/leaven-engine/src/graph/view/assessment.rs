use leaven_core::{AssessmentTarget, EvaluationRequest, ResolvedEvaluationSet};
use leaven_kernel::{
    AssessmentId, CandidateId, EvaluationRequestId, EvaluatorId, EvidenceRef, Fingerprint,
    MetadataBag, Timestamp,
};

use crate::graph::storage::{AssessmentRecord, AssessmentRecordTarget, EvaluationRequestRecord};

pub struct AssessmentView<'g> {
    pub(super) record: &'g AssessmentRecord,
}

impl AssessmentView<'_> {
    #[must_use]
    pub fn id(&self) -> AssessmentId {
        self.record.id
    }

    #[must_use]
    pub fn request_id(&self) -> EvaluationRequestId {
        self.record.request_id
    }

    #[must_use]
    pub fn evidence_ref(&self) -> &EvidenceRef {
        &self.record.evidence
    }

    #[must_use]
    pub fn evaluator(&self) -> &EvaluatorId {
        &self.record.evaluator
    }

    #[must_use]
    pub fn metadata(&self) -> &MetadataBag {
        &self.record.metadata
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.record.created_at
    }

    #[must_use]
    pub fn independent_candidate(&self) -> Option<CandidateId> {
        match &self.record.target {
            AssessmentRecordTarget::Independent { candidate, .. } => Some(*candidate),
            AssessmentRecordTarget::Pairwise { .. } | AssessmentRecordTarget::Listwise { .. } => {
                None
            }
        }
    }

    #[must_use]
    pub fn target(&self) -> &AssessmentTarget {
        match &self.record.target {
            AssessmentRecordTarget::Independent { target, .. }
            | AssessmentRecordTarget::Pairwise { target, .. }
            | AssessmentRecordTarget::Listwise { target, .. } => target,
        }
    }

    #[must_use]
    pub fn pairwise_candidates(&self) -> Option<(CandidateId, CandidateId)> {
        match &self.record.target {
            AssessmentRecordTarget::Pairwise { left, right, .. } => Some((*left, *right)),
            AssessmentRecordTarget::Independent { .. }
            | AssessmentRecordTarget::Listwise { .. } => None,
        }
    }

    #[must_use]
    pub fn listwise_candidates(&self) -> Option<&[CandidateId]> {
        match &self.record.target {
            AssessmentRecordTarget::Listwise { candidates, .. } => Some(candidates),
            AssessmentRecordTarget::Independent { .. }
            | AssessmentRecordTarget::Pairwise { .. } => None,
        }
    }
}

pub struct AssessmentQuery<'g> {
    pub(super) assessments: Vec<AssessmentView<'g>>,
}

impl<'g> AssessmentQuery<'g> {
    #[must_use]
    pub fn len(&self) -> usize {
        self.assessments.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.assessments.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &AssessmentView<'g>> {
        self.assessments.iter()
    }

    #[must_use]
    pub fn ids(&self) -> Vec<AssessmentId> {
        self.assessments.iter().map(AssessmentView::id).collect()
    }
}

pub struct EvaluationRequestView<'g> {
    pub(super) record: &'g EvaluationRequestRecord,
}

impl EvaluationRequestView<'_> {
    #[must_use]
    pub fn id(&self) -> EvaluationRequestId {
        self.record.id
    }

    #[must_use]
    pub fn evaluator(&self) -> &EvaluatorId {
        &self.record.evaluator
    }

    #[must_use]
    pub fn evaluator_fingerprint(&self) -> Fingerprint {
        self.record.evaluator_fingerprint
    }

    #[must_use]
    pub fn request(&self) -> &EvaluationRequest {
        &self.record.request
    }

    #[must_use]
    pub fn resolved_set(&self) -> &ResolvedEvaluationSet {
        &self.record.resolved_set
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.record.created_at
    }
}
