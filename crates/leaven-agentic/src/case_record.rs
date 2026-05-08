//! Durable case-run policy and record vocabulary.

use std::num::NonZeroUsize;
use std::time::Duration;

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
    pub retries: Vec<AgentCaseRetryRecord>,
    pub cost: Cost,
}

impl AgentCaseRunRecord {
    /// Constructs a successful scored run record.
    #[must_use]
    pub fn scored_attempt(input: ScoredAgentCaseRun) -> Self {
        Self {
            run_id: input.run_id,
            candidate: input.candidate,
            case: input.case,
            partition: input.partition,
            attempt: input.attempt,
            session: Some(input.session),
            outputs: input.outputs,
            score_recorded: true,
            error: None,
            retries: input.retries,
            cost: input.cost,
        }
    }

    /// Constructs an unscored failed run record.
    #[must_use]
    pub fn failed_attempt(input: FailedAgentCaseRun) -> Self {
        Self {
            run_id: input.run_id,
            candidate: input.candidate,
            case: input.case,
            partition: input.partition,
            attempt: input.attempt,
            session: input.session,
            outputs: input.outputs,
            score_recorded: false,
            error: Some(input.error),
            retries: Vec::new(),
            cost: input.cost,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScoredAgentCaseRun {
    pub run_id: RunId,
    pub candidate: CandidateId,
    pub case: CaseId,
    pub partition: EvaluationSetId,
    pub attempt: NonZeroUsize,
    pub session: AgentSessionId,
    pub outputs: Vec<WorkspacePath>,
    pub retries: Vec<AgentCaseRetryRecord>,
    pub cost: Cost,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FailedAgentCaseRun {
    pub run_id: RunId,
    pub candidate: CandidateId,
    pub case: CaseId,
    pub partition: EvaluationSetId,
    pub attempt: NonZeroUsize,
    pub session: Option<AgentSessionId>,
    pub outputs: Vec<WorkspacePath>,
    pub error: AgentCaseRunError,
    pub cost: Cost,
}

/// Compact retry history embedded in the completed case-run record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentCaseRetryRecord {
    pub attempt: NonZeroUsize,
    pub session: Option<AgentSessionId>,
    pub error: AgentCaseRunError,
    pub cost: Cost,
}

impl AgentCaseRetryRecord {
    #[must_use]
    pub fn from_failed_run(record: &AgentCaseRunRecord) -> Option<Self> {
        Some(Self {
            attempt: record.attempt,
            session: record.session,
            error: record.error.clone()?,
            cost: record.cost.clone(),
        })
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

/// Case execution policy for agentic evaluators.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentCaseRunPolicy {
    pub retry_on_error: usize,
    pub score_on_error: bool,
    pub fail_on_error: FailOnError,
    pub max_parallel_cases: Option<NonZeroUsize>,
    pub max_parallel_workspaces: Option<NonZeroUsize>,
    pub limits: AgentCaseLimits,
    pub approval: Option<ToolApprovalPolicy>,
    pub checkpoint: CaseCheckpointPolicy,
}

impl Default for AgentCaseRunPolicy {
    fn default() -> Self {
        Self {
            retry_on_error: 0,
            score_on_error: false,
            fail_on_error: FailOnError::Any,
            max_parallel_cases: None,
            max_parallel_workspaces: None,
            limits: AgentCaseLimits::default(),
            approval: None,
            checkpoint: CaseCheckpointPolicy::default(),
        }
    }
}

/// Policy for when evaluator errors abort the surrounding evaluation request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FailOnError {
    Any,
    Never,
    Count(NonZeroUsize),
    Fraction(FiniteRatio),
}

/// A positive finite ratio used by failure policy thresholds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FiniteRatio {
    numerator: NonZeroUsize,
    denominator: NonZeroUsize,
}

impl FiniteRatio {
    #[must_use]
    pub const fn new(numerator: NonZeroUsize, denominator: NonZeroUsize) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    #[must_use]
    pub const fn numerator(self) -> NonZeroUsize {
        self.numerator
    }

    #[must_use]
    pub const fn denominator(self) -> NonZeroUsize {
        self.denominator
    }
}

/// Per-case limits that can be translated into provider/runtime limits.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AgentCaseLimits {
    pub message_limit: Option<NonZeroUsize>,
    pub token_limit: Option<NonZeroUsize>,
    pub time_limit: Option<Duration>,
    pub working_time_limit: Option<Duration>,
    pub cost_limit: Option<Cost>,
}

/// Tool approval policy requested by an agentic case run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolApprovalPolicy {
    pub require_approval: bool,
    pub allowed_tools: Vec<String>,
}

/// Checkpoint expectation for case-run records.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CaseCheckpointPolicy {
    Disabled,
    BestEffort,
    #[default]
    BeforeAssessment,
}
