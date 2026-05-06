mod support;

use futures::executor::block_on;
use leaven_core::{
    Assessment, AssessmentGranularity, AssessmentTarget, EvaluationPurpose, EvaluationSet,
    Preference, Proposal, ProposalBatch, ProposalBatchSemantics, ResolvedEvaluationRequest,
};
use leaven_engine::{
    Arity, CachePolicy, DynEvaluator, DynPreferenceRelation, DynProposer, DynStopper,
    EvaluationContext, EvaluationError, Evaluator, PreferenceRelation, ProposalContext,
    ProposalError, Proposer, RunContext, Stopper,
};
use leaven_kernel::{Cost, EvaluatorId, Fingerprint, MetadataBag, Metered, ProposerId, StageId};

use support::{TestEvidence, TestProblem, TextArtifact, graph_and_budget};

#[test]
fn dyn_proposer_delegates_to_static_proposer() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let proposer = ContractProposer;
        let stage = StageId::from_proposer(ProposerId::from("contract"));
        let proposal_ctx = ctx.proposal_context(stage);
        let dyn_proposer: &dyn DynProposer<TestProblem> = &proposer;

        let report = dyn_proposer
            .propose_boxed(Box::new("candidate"), proposal_ctx)
            .await
            .unwrap();

        assert_eq!(dyn_proposer.id(), ProposerId::from("contract"));
        assert_eq!(dyn_proposer.arity(), Arity::Single);
        assert_eq!(report.value.proposals.len(), 1);
    });
}

#[test]
fn dyn_proposer_reports_request_type_mismatch_as_typed_error() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let proposer = ContractProposer;
        let proposal_ctx = ctx.proposal_context(StageId::from_proposer(Proposer::id(&proposer)));
        let dyn_proposer: &dyn DynProposer<TestProblem> = &proposer;

        let result = dyn_proposer
            .propose_boxed(Box::new(42_u64), proposal_ctx)
            .await;
        let Err(err) = result else {
            panic!("wrong erased request type must fail");
        };

        assert!(matches!(
            err,
            ProposalError::RequestTypeMismatch {
                proposer,
                expected,
            } if proposer == ProposerId::from("contract") && expected == "&str"
        ));
    });
}

#[test]
fn dyn_evaluator_delegates_to_static_evaluator() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let candidate = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact("abc".to_owned()), 0).unwrap()
        };
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let evaluator = ContractEvaluator;
        let eval_ctx = ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator)));
        let request = ResolvedEvaluationRequest {
            kind: leaven_core::ResolvedRequestKind::Independent {
                candidates: vec![candidate],
            },
            set: leaven_core::ResolvedEvaluationSet {
                id: leaven_kernel::ResolvedEvaluationSetId::new(),
                expr: EvaluationSet::All,
                case_ids: Vec::new(),
                resolved_at: leaven_kernel::now(),
                case_set_version: leaven_core::CaseSetVersion("contract".to_owned()),
            },
            granularity: AssessmentGranularity::Aggregate,
            purpose: EvaluationPurpose::Search,
        };
        let dyn_evaluator: &dyn DynEvaluator<TestProblem> = &evaluator;

        let report = dyn_evaluator
            .evaluate_boxed(request, eval_ctx)
            .await
            .unwrap();

        assert_eq!(dyn_evaluator.id(), EvaluatorId::from("contract"));
        assert_eq!(report.value.len(), 1);
    });
}

#[test]
fn dyn_preference_and_stopper_delegate_to_static_traits() {
    let (mut graph, mut budget) = graph_and_budget();
    let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let preference: &dyn DynPreferenceRelation<TestProblem> = &AlwaysLeft;
    let stopper: &dyn DynStopper<TestProblem> = &StopWhenEmpty;

    assert_eq!(
        preference.prefer_dyn(
            leaven_kernel::CandidateId::new(),
            leaven_kernel::CandidateId::new(),
            ctx.graph(),
        ),
        Preference::LeftBetter
    );
    assert!(stopper.should_stop_dyn(ctx.graph()));
}

struct ContractProposer;

impl Proposer<TestProblem> for ContractProposer {
    type Request = &'static str;

    fn id(&self) -> ProposerId {
        ProposerId::from("contract")
    }

    async fn propose(
        &self,
        request: Self::Request,
        _ctx: ProposalContext<'_, TestProblem>,
    ) -> Result<Metered<ProposalBatch<TestProblem>>, ProposalError> {
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![Proposal::create(TextArtifact(request.to_owned())).build()],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        ))
    }
}

struct ContractEvaluator;

impl Evaluator<TestProblem> for ContractEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::from("contract")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([3; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        let leaven_core::ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message("expected independent".to_owned()));
        };
        Ok(Metered::new(
            candidates
                .into_iter()
                .map(|candidate| Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Unscoped,
                    evidence: TestEvidence { score: 1.0 },
                    cost: Cost::zero(),
                    metadata: MetadataBag::new(),
                })
                .collect(),
            Cost::zero(),
        ))
    }
}

struct AlwaysLeft;

impl PreferenceRelation<TestProblem> for AlwaysLeft {
    fn prefer(
        &self,
        _left: leaven_kernel::CandidateId,
        _right: leaven_kernel::CandidateId,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Preference {
        Preference::LeftBetter
    }
}

struct StopWhenEmpty;

impl Stopper<TestProblem> for StopWhenEmpty {
    fn should_stop(&self, graph: leaven_engine::RunGraphView<'_, TestProblem>) -> bool {
        graph.candidate_count() == 0
    }
}
