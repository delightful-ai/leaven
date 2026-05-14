//! GEPA reflection routing through optimizer-stage agent workspaces.

use leaven_core::{InfoRef, OptimizationProblem};
use leaven_kernel::{AssessmentId, CandidateId, StageRole};
use leaven_stage::{
    AgentBacked, AgentBackedPolicy, AgentStageBootstrap, AgentStageCallContext, AgentStagePlan,
    AllowedQuerySet, ProposerSlot, StageBootstrapError, StageDirective, StageOutputContract,
    StageQuery, StageQueryKind, StageQueryPolicy,
};
use leaven_workspace::{WorkspaceFactory, WorkspacePath};

pub type GepaStageProposer<Runtime, Parser> =
    AgentBacked<ProposerSlot<ReflectRequest>, Runtime, GepaReflectionBootstrap, Parser>;

#[must_use]
pub fn gepa_stage_proposer<Factory, Runtime, Parser>(
    workspace_factory: Factory,
    runtime: Runtime,
    parser: Parser,
    policy: AgentBackedPolicy,
) -> GepaStageProposer<Runtime, Parser>
where
    Factory: WorkspaceFactory + Send + Sync + 'static,
{
    AgentBacked::from_factory(
        workspace_factory,
        runtime,
        GepaReflectionBootstrap::default(),
        parser,
        policy,
    )
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SelectedFeedback {
    pub assessment_refs: Vec<AssessmentId>,
    pub evidence_refs: Vec<InfoRef>,
    pub candidate_refs: Vec<CandidateId>,
}

impl SelectedFeedback {
    #[must_use]
    pub fn with_assessments(mut self, feedback: impl IntoIterator<Item = AssessmentId>) -> Self {
        self.assessment_refs.extend(feedback);
        self
    }

    #[must_use]
    pub fn source_refs(&self) -> Vec<InfoRef> {
        self.candidate_refs
            .iter()
            .copied()
            .map(InfoRef::Candidate)
            .chain(
                self.assessment_refs
                    .iter()
                    .copied()
                    .map(InfoRef::Assessment),
            )
            .chain(self.evidence_refs.iter().cloned())
            .collect()
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ReflectRequest {
    pub parent: CandidateId,
    pub part_label: String,
    pub selected_feedback: SelectedFeedback,
}

impl ReflectRequest {
    #[must_use]
    pub fn new(parent: CandidateId, part_label: impl Into<String>) -> Self {
        Self {
            parent,
            part_label: part_label.into(),
            selected_feedback: SelectedFeedback::default(),
        }
    }

    #[must_use]
    pub fn with_feedback(mut self, feedback: impl IntoIterator<Item = AssessmentId>) -> Self {
        self.selected_feedback = self.selected_feedback.with_assessments(feedback);
        self
    }

    #[must_use]
    pub fn with_selected_feedback(mut self, selected_feedback: SelectedFeedback) -> Self {
        self.selected_feedback = selected_feedback;
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

impl<P> AgentStageBootstrap<P, ProposerSlot<ReflectRequest>> for GepaReflectionBootstrap
where
    P: OptimizationProblem,
{
    async fn plan(
        &self,
        request: ReflectRequest,
        _ctx: AgentStageCallContext,
    ) -> Result<AgentStagePlan<ReflectRequest>, StageBootstrapError> {
        let mut prewarm = vec![StageQuery::Candidate { id: request.parent }];
        prewarm.extend(
            request
                .selected_feedback
                .assessment_refs
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
