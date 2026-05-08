//! Skill agentic input types.

use leaven_kernel::CandidateId;

/// Parent candidate whose skill bank is materialized and mutated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillBankProposalInput {
    /// Candidate id of the parent `SkillBank`.
    pub parent: CandidateId,
}

impl SkillBankProposalInput {
    /// Constructs a skill-bank proposal input.
    #[must_use]
    pub const fn new(parent: CandidateId) -> Self {
        Self { parent }
    }
}
