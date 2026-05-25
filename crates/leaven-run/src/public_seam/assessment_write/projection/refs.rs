use std::collections::BTreeSet;

use leaven_kernel::{AssessmentId, CandidateId, EvaluationRequestId};

pub(in crate::public_seam::assessment_write) fn sorted_assessment_refs(
    ids: &[AssessmentId],
) -> Vec<String> {
    ids.iter()
        .copied()
        .map(assessment_ref)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn assessment_ref(id: AssessmentId) -> String {
    uuid_ref("assess", id.as_uuid())
}

pub(super) fn candidate_ref(id: CandidateId) -> String {
    uuid_ref("cand", id.as_uuid())
}

pub(super) fn candidate_refs(ids: &[CandidateId]) -> Vec<String> {
    ids.iter().copied().map(candidate_ref).collect()
}

pub(in crate::public_seam::assessment_write) fn evaluation_request_ref(
    id: EvaluationRequestId,
) -> String {
    uuid_ref("evalreq", id.as_uuid())
}

fn uuid_ref(prefix: &str, id: uuid::Uuid) -> String {
    format!("{prefix}_{id}")
}
