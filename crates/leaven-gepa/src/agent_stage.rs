//! GEPA reflection routing through optimizer-stage agent workspaces.

use leaven_core::OptimizationProblem;
use leaven_kernel::{AssessmentId, CandidateId, StageRole};
use leaven_stage::{
    AgentStageBootstrap, AgentStageCallContext, AgentStagePlan, AllowedQuerySet, ProposerSlot,
    StageBootstrapError, StageDirective, StageOutputContract, StageQuery, StageQueryKind,
    StageQueryPolicy,
};
use leaven_workspace::WorkspacePath;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GepaReflectionRequest {
    pub parent: CandidateId,
    pub part_label: String,
    pub feedback: Vec<AssessmentId>,
}

impl GepaReflectionRequest {
    #[must_use]
    pub fn new(parent: CandidateId, part_label: impl Into<String>) -> Self {
        Self {
            parent,
            part_label: part_label.into(),
            feedback: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_feedback(mut self, feedback: impl IntoIterator<Item = AssessmentId>) -> Self {
        self.feedback.extend(feedback);
        self
    }
}

#[derive(Clone, Debug)]
pub struct GepaReflectionBootstrap {
    output_path: WorkspacePath,
}

impl GepaReflectionBootstrap {
    #[must_use]
    pub fn new(output_path: WorkspacePath) -> Self {
        Self { output_path }
    }
}

impl Default for GepaReflectionBootstrap {
    fn default() -> Self {
        Self::new(WorkspacePath::new("output/proposal.json").expect("static path"))
    }
}

impl<P> AgentStageBootstrap<P, ProposerSlot<GepaReflectionRequest>> for GepaReflectionBootstrap
where
    P: OptimizationProblem,
{
    async fn plan(
        &self,
        request: GepaReflectionRequest,
        _ctx: AgentStageCallContext,
    ) -> Result<AgentStagePlan<GepaReflectionRequest>, StageBootstrapError> {
        let mut prewarm = vec![StageQuery::Candidate { id: request.parent }];
        prewarm.extend(
            request
                .feedback
                .iter()
                .copied()
                .map(|id| StageQuery::Assessment { id }),
        );
        Ok(AgentStagePlan::new(
            StageRole::reflect(),
            request,
            StageDirective::new(
                "GEPA reflection",
                "Use the parent candidate and selected feedback to write a proposal JSON file.",
            ),
            StageOutputContract::proposal_json(self.output_path.clone()),
        )
        .with_query_policy(StageQueryPolicy::bounded(
            AllowedQuerySet::only([
                StageQueryKind::Help,
                StageQueryKind::Candidate,
                StageQueryKind::Assessment,
                StageQueryKind::Lineage,
                StageQueryKind::Diff,
            ]),
            prewarm,
            Some(64),
            Some(64 * 1024 * 1024),
        )))
    }
}
