//! Proposal repair feedback for agentic proposers.

use std::num::NonZeroUsize;

use leaven_agent::{AgentInstructions, AgentSession};

use crate::{AgenticParseError, AgenticRepairError};

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

pub trait ProposalRepairPromptBuilder<I>: Send + Sync {
    fn build_repair(
        &self,
        input: &I,
        feedback: ProposalRepairFeedback<'_>,
    ) -> Result<AgentInstructions, AgenticRepairError>;
}
