use leaven_kernel::EvidenceRef;

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct EvoSkillProposalAnnotations {
    pub proposal: Option<SkillProposal>,
    pub proposer_evidence: Option<EvidenceRef>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct SkillProposal {
    pub action: SkillAction,
    pub target_skill: Option<String>,
    pub proposed_skill: String,
    pub justification: String,
    #[serde(default)]
    pub related_iterations: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillAction {
    Create,
    Edit,
}
