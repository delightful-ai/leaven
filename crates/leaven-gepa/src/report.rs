//! GEPA report snapshots.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use leaven_kernel::{AssessmentId, CandidateId, CaseId};
use serde::{Deserialize, Serialize};

use crate::{
    GepaCandidateHistoryEntry, GepaCandidateIndex, GepaEventSummary, GepaProposalAttempt,
    GepaReferenceState,
};

/// Detailed GEPA optimizer report.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GepaReport {
    /// Effective GEPA profile label and report-relevant knobs.
    pub profile: GepaReportProfile,
    /// Best candidate index selected by GEPA validation/frontier state.
    pub best_index: Option<GepaCandidateIndex>,
    /// Best candidate id selected by GEPA validation/frontier state.
    pub best_candidate: Option<CandidateId>,
    /// Best validation candidate index.
    pub validation_best_index: Option<GepaCandidateIndex>,
    /// Best validation candidate id.
    pub validation_best_candidate: Option<CandidateId>,
    /// Accepted candidates in GEPA discovery order.
    pub candidates: Vec<GepaReportCandidate>,
    /// Per-validation-case frontier membership.
    pub validation_frontier: Vec<GepaReportFrontierCase>,
    /// Train-screening observations GEPA used during search.
    pub candidate_history: Vec<GepaReportHistoryEntry>,
    /// Proposal attempts GEPA made, including skipped and rejected attempts.
    pub proposal_attempts: Vec<GepaProposalAttempt>,
    /// Aggregate train-screen and validation outcomes for proposal attempts.
    pub quality_summary: GepaReportQualitySummary,
    /// Total new evaluator metric calls charged during GEPA search.
    pub total_metric_calls: u64,
    /// Number of full validation passes run by GEPA.
    pub full_validation_evals: u64,
    /// Whether all-perfect parent minibatches are skipped before reflection.
    pub skip_perfect_score: bool,
    /// Score threshold treated as perfect by the skip-perfect policy.
    pub perfect_score: f64,
    /// GEPA phase events emitted by the optimizer.
    pub events: Vec<GepaEventSummary>,
}

impl GepaReport {
    pub(crate) fn from_reference_state(input: &GepaReportInput<'_>) -> Self {
        let reference_state = input.reference_state;
        let best_index = input
            .best_candidate
            .and_then(|candidate| reference_state.index_of(candidate));
        let validation_best_index = input
            .validation_best_candidate
            .and_then(|candidate| reference_state.index_of(candidate));
        let candidates = reference_state
            .records()
            .iter()
            .enumerate()
            .map(|(ordinal, record)| GepaReportCandidate {
                index: record.index(),
                candidate: record.candidate(),
                parents: record.parents().to_vec(),
                discovery_metric_calls: record.discovery_metric_calls(),
                validation_score: record.validation_score(),
                validation_rows: record.validation_rows().to_vec(),
                validation_subscores: reference_state
                    .validation_subscores()
                    .get(ordinal)
                    .into_iter()
                    .flat_map(|rows| rows.iter())
                    .map(|(case, score)| GepaReportValidationSubscore {
                        case: *case,
                        score: score.score(),
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        let validation_frontier = reference_state
            .validation_frontier()
            .iter()
            .map(|(case, candidates)| GepaReportFrontierCase {
                case: *case,
                candidates: candidates.iter().copied().collect(),
            })
            .collect();
        let candidate_history = input
            .candidate_history
            .iter()
            .map(|entry| GepaReportHistoryEntry {
                candidate: entry.candidate(),
                candidate_index: reference_state.index_of(entry.candidate()),
                assessments: entry.assessments().to_vec(),
                score: entry.score(),
            })
            .collect();
        let proposal_attempts = input.proposal_attempts.to_vec();
        let quality_summary =
            GepaReportQualitySummary::from_report_rows(&proposal_attempts, &candidates);
        Self {
            profile: input.profile.clone(),
            best_index,
            best_candidate: input.best_candidate,
            validation_best_index,
            validation_best_candidate: input.validation_best_candidate,
            candidates,
            validation_frontier,
            candidate_history,
            proposal_attempts,
            quality_summary,
            total_metric_calls: reference_state.total_metric_calls(),
            full_validation_evals: reference_state.full_validation_evals(),
            skip_perfect_score: input.skip_perfect_score,
            perfect_score: input.perfect_score,
            events: input.events.to_vec(),
        }
    }
}

/// Aggregate proposal quality counters for a GEPA report.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GepaReportQualitySummary {
    /// Total proposal attempts, including skipped attempts.
    pub proposal_attempt_count: usize,
    /// Attempts skipped before a child was produced or screened.
    pub skipped_count: usize,
    /// Attempts with both parent and child train-screening scores.
    pub screened_count: usize,
    /// Screened attempts where child train score exceeded parent train score.
    pub screened_train_improved_count: usize,
    /// Screened attempts where child train score tied parent train score.
    pub screened_train_tied_count: usize,
    /// Screened attempts where child train score trailed parent train score.
    pub screened_train_regressed_count: usize,
    /// Train-screened attempts accepted by the configured gate.
    pub accepted_count: usize,
    /// Train-screened attempts rejected by the configured gate.
    pub rejected_count: usize,
    /// Accepted attempts admitted into the GEPA reference state.
    pub admitted_count: usize,
    /// Accepted attempts not admitted, usually because the run stopped first.
    pub accepted_unadmitted_count: usize,
    /// Accepted admitted attempts whose full validation beat the parent.
    pub accepted_validation_improved_count: usize,
    /// Accepted admitted attempts whose full validation tied the parent.
    pub accepted_validation_tied_count: usize,
    /// Accepted admitted attempts whose full validation trailed the parent.
    pub accepted_validation_regressed_count: usize,
    /// Accepted attempts whose validation comparison is unavailable.
    pub accepted_validation_unknown_count: usize,
}

impl GepaReportQualitySummary {
    fn from_report_rows(
        attempts: &[GepaProposalAttempt],
        candidates: &[GepaReportCandidate],
    ) -> Self {
        let validation_scores = candidates
            .iter()
            .map(|candidate| (candidate.index, candidate.validation_score))
            .collect::<BTreeMap<_, _>>();
        let mut summary = Self {
            proposal_attempt_count: attempts.len(),
            skipped_count: attempts
                .iter()
                .filter(|attempt| attempt.skip_reason.is_some())
                .count(),
            ..Self::default()
        };

        for attempt in attempts {
            if let Some(child_score) = attempt.child_score {
                summary.screened_count += 1;
                match compare_scores(child_score, attempt.parent_score) {
                    Ordering::Greater => summary.screened_train_improved_count += 1,
                    Ordering::Equal => summary.screened_train_tied_count += 1,
                    Ordering::Less => summary.screened_train_regressed_count += 1,
                }
            }

            match attempt.accepted {
                Some(true) => {
                    summary.accepted_count += 1;
                    if let Some(child_index) = attempt.admitted_index {
                        summary.admitted_count += 1;
                        match (
                            validation_scores
                                .get(&attempt.parent_index)
                                .copied()
                                .flatten(),
                            validation_scores.get(&child_index).copied().flatten(),
                        ) {
                            (Some(parent_score), Some(child_score)) => {
                                match compare_scores(child_score, parent_score) {
                                    Ordering::Greater => {
                                        summary.accepted_validation_improved_count += 1;
                                    }
                                    Ordering::Equal => summary.accepted_validation_tied_count += 1,
                                    Ordering::Less => {
                                        summary.accepted_validation_regressed_count += 1;
                                    }
                                }
                            }
                            _ => summary.accepted_validation_unknown_count += 1,
                        }
                    } else {
                        summary.accepted_unadmitted_count += 1;
                        summary.accepted_validation_unknown_count += 1;
                    }
                }
                Some(false) => summary.rejected_count += 1,
                None => {}
            }
        }

        summary
    }
}

fn compare_scores(left: f64, right: f64) -> Ordering {
    if (left - right).abs() <= f64::EPSILON {
        Ordering::Equal
    } else if left > right {
        Ordering::Greater
    } else {
        Ordering::Less
    }
}

pub(crate) struct GepaReportInput<'a> {
    pub(crate) profile: &'a GepaReportProfile,
    pub(crate) reference_state: &'a GepaReferenceState,
    pub(crate) candidate_history: &'a [GepaCandidateHistoryEntry],
    pub(crate) proposal_attempts: &'a [GepaProposalAttempt],
    pub(crate) events: &'a [GepaEventSummary],
    pub(crate) best_candidate: Option<CandidateId>,
    pub(crate) validation_best_candidate: Option<CandidateId>,
    pub(crate) skip_perfect_score: bool,
    pub(crate) perfect_score: f64,
}

/// Effective GEPA profile facts reported with every run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GepaReportProfile {
    /// Stable profile label after public builder overrides are applied.
    pub label: String,
    /// Train minibatch size when the sampler is a known GEPA profile sampler.
    pub train_minibatch_size: Option<usize>,
    /// Serial proposal attempts per selected parent/minibatch.
    pub proposal_count: usize,
    /// Proposal scheduling mode.
    pub proposal_mode: String,
    /// Validation/admission policy label.
    pub validation_policy: String,
    /// Certification mode reported for accepted candidates.
    pub certification_mode: String,
    /// Whether all-perfect parent minibatches are skipped before reflection.
    pub skip_perfect_score: bool,
    /// Score threshold treated as perfect by the skip-perfect policy.
    pub perfect_score: String,
}

impl Default for GepaReportProfile {
    fn default() -> Self {
        Self {
            label: "reference".to_owned(),
            train_minibatch_size: Some(3),
            proposal_count: 1,
            proposal_mode: "serial".to_owned(),
            validation_policy: "full-validation".to_owned(),
            certification_mode: "full-validation-before-admission".to_owned(),
            skip_perfect_score: true,
            perfect_score: "1".to_owned(),
        }
    }
}

/// One accepted candidate row in a GEPA report.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GepaReportCandidate {
    /// GEPA discovery-order index.
    pub index: GepaCandidateIndex,
    /// Candidate id in graph truth.
    pub candidate: CandidateId,
    /// Parent GEPA indices.
    pub parents: Vec<GepaCandidateIndex>,
    /// Metric calls spent when this candidate was admitted.
    pub discovery_metric_calls: u64,
    /// Aggregate validation score.
    pub validation_score: Option<f64>,
    /// Validation assessment rows backing this candidate.
    pub validation_rows: Vec<AssessmentId>,
    /// Per-case validation scores for this candidate.
    pub validation_subscores: Vec<GepaReportValidationSubscore>,
}

/// Per-case validation subscore for a candidate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GepaReportValidationSubscore {
    /// Validation case id.
    pub case: CaseId,
    /// Scalar validation score.
    pub score: f64,
}

/// Frontier membership for one validation case.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GepaReportFrontierCase {
    /// Validation case id.
    pub case: CaseId,
    /// Candidate indices tied for this case frontier.
    pub candidates: Vec<GepaCandidateIndex>,
}

/// One train-screening observation in GEPA history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GepaReportHistoryEntry {
    /// Candidate id in graph truth.
    pub candidate: CandidateId,
    /// GEPA candidate index, when the observation belongs to admitted state.
    pub candidate_index: Option<GepaCandidateIndex>,
    /// Assessment rows used for this observation.
    pub assessments: Vec<AssessmentId>,
    /// Average train-screening score.
    pub score: f64,
}
