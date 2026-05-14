use futures::executor::block_on;
use leaven_core::{Artifact, ArtifactIdentity, CacheIdentity, Evidence, OptimizationProblem};
use leaven_gepa::{GepaReflectionBootstrap, GepaReflectionRequest};
use leaven_kernel::{AssessmentId, BudgetSnapshot, CandidateId, ContentId, StageCallId, StageRole};
use leaven_stage::{
    AgentStageBootstrap, AgentStageCallContext, ProposerSlot, StageQuery, StageQueryKind,
};

#[test]
fn gepa_reflection_bootstrap_prewarms_parent_and_feedback_queries() {
    block_on(async {
        let parent = CandidateId::new();
        let feedback = AssessmentId::new();
        let request = GepaReflectionRequest::new(parent, "answer").with_feedback([feedback]);
        let ctx = AgentStageCallContext::new(
            StageCallId::new(),
            leaven_engine::ReadScope::default(),
            BudgetSnapshot::default(),
        );

        let plan = <GepaReflectionBootstrap as AgentStageBootstrap<
            TestProblem,
            ProposerSlot<GepaReflectionRequest>,
        >>::plan(&GepaReflectionBootstrap::default(), request, ctx)
        .await
        .unwrap();

        assert_eq!(plan.role, StageRole::reflect());
        assert!(plan.query.allowed.contains(StageQueryKind::Candidate));
        assert_eq!(plan.query.prewarm[0], StageQuery::Candidate { id: parent });
        assert_eq!(
            plan.query.prewarm[1],
            StageQuery::Assessment { id: feedback }
        );
    });
}

#[derive(Clone, Debug)]
struct TestArtifact;

impl Artifact for TestArtifact {
    type Change = ();
    type ApplyError = std::convert::Infallible;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::from_bytes([1; 32]))
    }

    fn cache_identity(&self) -> Option<CacheIdentity> {
        Some(CacheIdentity::Content(ContentId::from_bytes([1; 32])))
    }

    fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self)
    }
}

#[derive(Clone, Debug)]
struct TestEvidence;

impl Evidence for TestEvidence {}

struct TestProblem;

impl OptimizationProblem for TestProblem {
    type Artifact = TestArtifact;
    type Case = ();
    type Evidence = TestEvidence;
    type ProposalAnnotations = ();
}
