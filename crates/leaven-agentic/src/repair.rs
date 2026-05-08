//! Proposal repair feedback for agentic proposers.

use std::num::NonZeroUsize;

use leaven_agent::{AgentInstructions, AgentSession};
use leaven_kernel::AgentSessionId;
use serde::{Deserialize, Serialize};

use crate::{AgenticParseError, AgenticRepairError};

pub const PROPOSAL_REPAIR_ATTEMPTS_METADATA_KEY: &str = "leaven.agentic.proposal_repair_attempts";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProposalRepairPolicy {
    pub max_attempts: NonZeroUsize,
}

impl ProposalRepairPolicy {
    #[must_use]
    pub const fn new(max_attempts: NonZeroUsize) -> Self {
        Self { max_attempts }
    }
}

impl Default for ProposalRepairPolicy {
    fn default() -> Self {
        Self {
            max_attempts: NonZeroUsize::new(2).expect("2 is non-zero"),
        }
    }
}

#[derive(Debug)]
pub struct ProposalRepairFeedback<'a> {
    pub failed_attempt: NonZeroUsize,
    pub max_attempts: NonZeroUsize,
    pub parse_error: &'a AgenticParseError,
    pub previous_session: &'a AgentSession,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProposalRepairAttemptRecord {
    pub attempt: usize,
    pub session: AgentSessionId,
    pub outcome: ProposalRepairAttemptOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProposalRepairAttemptOutcome {
    Accepted,
    ParseFailed { error: String },
}

pub trait ProposalRepairPromptBuilder<I>: Send + Sync {
    fn build_repair(
        &self,
        input: &I,
        feedback: ProposalRepairFeedback<'_>,
    ) -> Result<AgentInstructions, AgenticRepairError>;
}
