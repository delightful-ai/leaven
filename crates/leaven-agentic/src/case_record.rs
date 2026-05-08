//! Durable case-run record vocabulary.

use std::num::NonZeroUsize;

use leaven_kernel::{AgentSessionId, CandidateId, CaseId, Cost, EvaluationSetId, RunId};
use leaven_workspace::WorkspacePath;
use serde::{Deserialize, Serialize};

/// Metadata key that stores a JSON-encoded [`AgentCaseRunRecord`].
pub const CASE_RUN_RECORD_METADATA_KEY: &str = "leaven.agentic.case_run_record";

/// Durable summary of one attempted agentic case execution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentCaseRunRecord {
    pub run_id: RunId,
    pub candidate: CandidateId,
    pub case: CaseId,
    pub partition: EvaluationSetId,
    pub attempt: NonZeroUsize,
    pub session: Option<AgentSessionId>,
    pub outputs: Vec<WorkspacePath>,
    pub score_recorded: bool,
    pub error: Option<AgentCaseRunError>,
    pub cost: Cost,
}

impl AgentCaseRunRecord {
    /// Constructs a successful scored run record.
    #[must_use]
    pub fn scored(
        run_id: RunId,
        candidate: CandidateId,
        case: CaseId,
        partition: EvaluationSetId,
        session: AgentSessionId,
        outputs: Vec<WorkspacePath>,
        cost: Cost,
    ) -> Self {
        Self {
            run_id,
            candidate,
            case,
            partition,
            attempt: NonZeroUsize::new(1).expect("literal one is non-zero"),
            session: Some(session),
            outputs,
            score_recorded: true,
            error: None,
            cost,
        }
    }
}

/// Structured failure family for an attempted case run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum AgentCaseRunError {
    Presentation(String),
    Runtime(String),
    Scoring(String),
    Cleanup(String),
}
