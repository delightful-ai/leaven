use leaven_artifact_skill::SkillBank;
use leaven_core::InfoRef;
use leaven_gepa::{ReflectRequest, ReflectiveCase};
use leaven_kernel::CandidateId;

/// Typed input for one skill-bank reflection workspace.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct SkillBankReflectionInput<Part = String> {
    pub parent: CandidateId,
    pub artifact: SkillBank,
    pub part: Part,
    pub part_label: String,
    pub examples: Vec<ReflectiveCase>,
    pub source_refs: Vec<InfoRef>,
    pub attempt_index: Option<usize>,
}

impl<Part> SkillBankReflectionInput<Part> {
    #[must_use]
    pub fn from_request(artifact: SkillBank, request: ReflectRequest<Part>) -> Self {
        Self {
            parent: request.parent,
            artifact,
            part: request.part,
            part_label: request.part_label,
            examples: request.examples,
            source_refs: request.source_refs,
            attempt_index: request.attempt_index,
        }
    }

    #[must_use]
    pub fn informed_by(&self) -> Vec<InfoRef> {
        self.source_refs
            .iter()
            .cloned()
            .chain(self.examples.iter().flat_map(|case| {
                case.source_refs
                    .iter()
                    .cloned()
                    .chain(case.runs.iter().flat_map(|run| run.source_refs.iter().cloned()))
            }))
            .collect()
    }
}
