//! Read-only inspection helpers for agentic run graphs.

use leaven_core::OptimizationProblem;
use leaven_engine::{CacheStatus, RunEvent, RunGraphView};
use leaven_kernel::{
    AssessmentId, BudgetSnapshot, CandidateId, Cost, EvaluationRequestId, EvaluatorId, MetadataKey,
    MetadataValue, ProposalBatchId, RunId, StageId,
};
use serde::{Deserialize, Serialize};

use crate::{
    AgentCaseRunRecord, CASE_RUN_RECORD_METADATA_KEY, PROPOSAL_REPAIR_ATTEMPTS_METADATA_KEY,
    ProposalRepairAttemptRecord,
};

/// Audit-oriented view over agentic metadata recorded in a run graph.
///
/// This is intentionally a view-builder, not graph truth. The graph remains
/// authoritative; this type extracts the standard agentic metadata keys into a
/// typed shape that UIs, examples, and paper reproductions can inspect without
/// hand-parsing JSON at every call site.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgenticRunInspection {
    pub run_id: RunId,
    pub best_candidate: Option<CandidateId>,
    pub best_lineage: Vec<CandidateId>,
    pub proposal_repairs: Vec<ProposalRepairInspection>,
    pub case_runs: Vec<AgentCaseRunRecord>,
    pub cache_events: Vec<AgenticCacheInspection>,
    pub costs: AgenticCostInspection,
    pub warnings: Vec<AgenticInspectionWarning>,
}

impl AgenticRunInspection {
    /// Builds an inspection view from the visible portion of a run graph.
    #[must_use]
    pub fn from_graph<P>(graph: &RunGraphView<'_, P>) -> Self
    where
        P: OptimizationProblem,
    {
        let run_id = graph.run_id();
        let mut warnings = Vec::new();
        let (best_candidate, final_budget) = latest_run_end(graph);
        let best_lineage = match best_candidate {
            Some(best) if graph.candidate(best).is_some() => {
                let mut lineage = vec![best];
                lineage.extend(graph.lineage(best).ancestors());
                lineage
            }
            Some(best) => {
                warnings.push(AgenticInspectionWarning::BestCandidateMissing { candidate: best });
                Vec::new()
            }
            None => Vec::new(),
        };

        let proposal_repairs = graph
            .proposal_batches()
            .filter_map(|batch| {
                let value = batch
                    .metadata()
                    .get(&MetadataKey::from(PROPOSAL_REPAIR_ATTEMPTS_METADATA_KEY))?;
                match parse_metadata::<Vec<ProposalRepairAttemptRecord>>(value) {
                    Ok(attempts) => Some(ProposalRepairInspection {
                        batch_id: batch.id(),
                        stage: batch.stage().clone(),
                        attempts,
                    }),
                    Err(reason) => {
                        warnings.push(AgenticInspectionWarning::MalformedProposalRepairMetadata {
                            batch_id: batch.id(),
                            reason,
                        });
                        None
                    }
                }
            })
            .collect();

        let mut case_runs = Vec::new();
        for assessment in graph.all_assessments() {
            let Some(value) = assessment
                .metadata()
                .get(&MetadataKey::from(CASE_RUN_RECORD_METADATA_KEY))
            else {
                continue;
            };
            match parse_metadata::<AgentCaseRunRecord>(value) {
                Ok(record) => case_runs.push(record),
                Err(reason) => {
                    warnings.push(AgenticInspectionWarning::MalformedCaseRunRecordMetadata {
                        assessment_id: assessment.id(),
                        reason,
                    });
                }
            }
        }

        let costs = AgenticCostInspection {
            final_budget,
            charged_events: charged_event_cost(graph),
            evaluation_events: evaluation_event_cost(graph),
            case_run_records: case_runs
                .iter()
                .fold(Cost::zero(), |total, record| total.combine(&record.cost)),
        };
        let cache_events = cache_events(graph);

        Self {
            run_id,
            best_candidate,
            best_lineage,
            proposal_repairs,
            case_runs,
            cache_events,
            costs,
            warnings,
        }
    }
}

/// Proposal repair attempts attached to one proposal batch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProposalRepairInspection {
    pub batch_id: ProposalBatchId,
    pub stage: StageId,
    pub attempts: Vec<ProposalRepairAttemptRecord>,
}

/// Cache decision attached to one completed evaluation event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgenticCacheInspection {
    pub request_id: EvaluationRequestId,
    pub evaluator: EvaluatorId,
    pub cache: CacheStatus,
}

/// Cost rollups available from graph events and agentic case records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgenticCostInspection {
    pub final_budget: Option<BudgetSnapshot>,
    pub charged_events: Cost,
    pub evaluation_events: Cost,
    pub case_run_records: Cost,
}

/// Non-fatal problems encountered while building an inspection view.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgenticInspectionWarning {
    BestCandidateMissing {
        candidate: CandidateId,
    },
    MalformedProposalRepairMetadata {
        batch_id: ProposalBatchId,
        reason: String,
    },
    MalformedCaseRunRecordMetadata {
        assessment_id: AssessmentId,
        reason: String,
    },
}

fn latest_run_end<P>(graph: &RunGraphView<'_, P>) -> (Option<CandidateId>, Option<BudgetSnapshot>)
where
    P: OptimizationProblem,
{
    let mut latest = (None, None);
    for event in graph.events() {
        if let RunEvent::OptimizationEnded { best, budget, .. } = event {
            latest = (*best, Some(budget.clone()));
        }
    }
    latest
}

fn charged_event_cost<P>(graph: &RunGraphView<'_, P>) -> Cost
where
    P: OptimizationProblem,
{
    graph
        .events()
        .fold(Cost::zero(), |total, event| match event {
            RunEvent::BudgetCharged { cost, .. } => total.combine(cost),
            _ => total,
        })
}

fn evaluation_event_cost<P>(graph: &RunGraphView<'_, P>) -> Cost
where
    P: OptimizationProblem,
{
    graph
        .events()
        .fold(Cost::zero(), |total, event| match event {
            RunEvent::EvaluationCompleted { cost, .. } => total.combine(cost),
            _ => total,
        })
}

fn cache_events<P>(graph: &RunGraphView<'_, P>) -> Vec<AgenticCacheInspection>
where
    P: OptimizationProblem,
{
    graph
        .events()
        .filter_map(|event| match event {
            RunEvent::EvaluationCompleted {
                request_id,
                evaluator,
                cache,
                ..
            } => Some(AgenticCacheInspection {
                request_id: *request_id,
                evaluator: evaluator.clone(),
                cache: *cache,
            }),
            _ => None,
        })
        .collect()
}

fn parse_metadata<T>(value: &MetadataValue) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    match value {
        MetadataValue::Json(value) => serde_json::from_value(value.clone())
            .map_err(|error| format!("metadata JSON did not match expected schema: {error}")),
        other => Err(format!("expected JSON metadata, got {other:?}")),
    }
}
