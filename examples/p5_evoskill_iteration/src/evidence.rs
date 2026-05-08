use leaven_agent::AgentSession;
use leaven_core::Evidence;
use leaven_kernel::CandidateId;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum EvoSkillEvidence {
    Evaluation {
        candidate: CandidateId,
        split: String,
        average_score: f64,
        cases: Vec<CaseExecution>,
    },
    AgentRoleSession {
        role: AgentRole,
        developer_instructions: String,
        session: AgentSession,
    },
}

impl Evidence for EvoSkillEvidence {}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct CaseExecution {
    pub case_id: String,
    pub question: String,
    pub expected_answer: String,
    pub predicted_answer: String,
    pub score: f64,
    pub passed: bool,
    pub developer_instructions: String,
    pub session: AgentSession,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentRole {
    Executor,
    Proposer,
    SkillBuilder,
}
