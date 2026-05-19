//! Legacy GEPA reflection routing through optimizer-stage agent workspaces.
//!
//! This module is explicit scaffold. It proves the old `AgentBacked` slot,
//! receipt, and provenance route, but it does not materialize the parent
//! artifact for the agent. Skill-bank agentic reflection uses the materializing
//! `GepaSkillBankAgenticReflector` path in `leaven-gepa-agentic-skill`.

use leaven_core::OptimizationProblem;
use leaven_kernel::StageRole;
use leaven_stage::{
    AgentBacked, AgentBackedPolicy, AgentStageBootstrap, AgentStageCallContext, AgentStagePlan,
    AllowedQuerySet, ProposerSlot, StageBootstrapError, StageDirective, StageOutputContract,
    StageQuery, StageQueryKind, StageQueryPolicy,
};
use leaven_workspace::{WorkspaceFactory, WorkspacePath};

use crate::reflection::ReflectRequest;

pub type GepaStageProposer<Runtime, Parser> =
    AgentBacked<ProposerSlot<ReflectRequest>, Runtime, GepaReflectionBootstrap, Parser>;

/// Builds the legacy `AgentBacked` GEPA stage scaffold.
///
/// This path is not the production skill-bank agentic reflection route because
/// it writes request metadata and query summaries, not the parent artifact. Use
/// `leaven_gepa_agentic_skill::GepaSkillBankAgenticReflector` for the
/// materializing skill-bank proposal-stage path.
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
        for source in request.informed_by() {
            match source {
                leaven_core::InfoRef::Candidate(id) => {
                    if id != request.parent {
                        prewarm.push(StageQuery::Candidate { id });
                    }
                }
                leaven_core::InfoRef::Assessment(id) => {
                    prewarm.push(StageQuery::Assessment { id });
                }
                leaven_core::InfoRef::Proposal(_) | leaven_core::InfoRef::External(_) => {}
            }
        }
        Ok(AgentStagePlan::new(
            StageRole::reflect(),
            request,
            StageDirective::new(
                "GEPA reflection",
                "Use the parent candidate and the reflective examples in the request to write a proposal JSON file.",
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
