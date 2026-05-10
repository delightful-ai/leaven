use std::collections::BTreeMap;

use futures::executor::block_on;
use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, Evidence, OptimizationProblem,
    ProposalBatch, ProposalBatchSemantics, ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_engine::{
    BudgetLedger, CachePolicy, CaseSet, EvaluationContext, EvaluationError, Evaluator,
    ProposalContext, ProposalError, Proposer, RunContext, RunGraph, TrustPolicy,
};
use leaven_gepa::{
    CandidateSelector, Gate, GateDecision, Gepa, ImprovementOrEqual, NoRegression,
    ParetoFrequencyWeighted, ReflectiveMutation, SelectBestCandidate, StrictImprovement,
    SurfaceProposer,
};
use leaven_kernel::{
    Budget, ContentId, Cost, EvaluatorId, Fingerprint, MetadataBag, Metered, ProposerId, RunId,
};
use leaven_population::{KeepBest, ParetoFrontier, TournamentPopulation};
use leaven_store_inline::InlineEvidenceStore;
use leaven_surface::{EditSurface, Part, PartAddress, SurfaceError, SurfaceFingerprint};
use proptest::prelude::*;

#[test]
fn gepa_owns_surface_and_lowers_selected_part_edits() {
    let artifact = PartMapArtifact(BTreeMap::from([
        ("answer".to_owned(), "draft".to_owned()),
        ("search".to_owned(), "query".to_owned()),
    ]));
    let mut gepa = Gepa::new(
        PartMapSurface,
        ParetoFrontier::by_case().build(),
        ReflectiveMutation::new("unused".to_owned()),
    );
    let mut proposer = ReflectiveMutation::new("improved".to_owned());

    let part = gepa.select_part(&artifact).unwrap();
    let edit = proposer
        .propose_edit(&artifact, gepa.surface(), &part)
        .unwrap();
    let change = gepa.change_part(&artifact, part.clone(), edit).unwrap();
    let changed = artifact.apply_change(&change).unwrap();

    assert_eq!(part, "answer");
    assert_eq!(artifact.0.get("answer").unwrap(), "draft");
    assert_eq!(changed.0.get("answer").unwrap(), "improved");
    assert_eq!(changed.0.get("search").unwrap(), "query");
}

proptest! {
    #[test]
    fn surface_lowering_then_apply_changes_only_selected_part(
        mutate_answer in any::<bool>(),
        answer in "[a-z]{0,16}",
        search in "[a-z]{0,16}",
        edit in "[a-z]{0,16}",
    ) {
        let artifact = PartMapArtifact(BTreeMap::from([
            ("answer".to_owned(), answer.clone()),
            ("search".to_owned(), search.clone()),
        ]));
        let gepa = Gepa::new(
            PartMapSurface,
            ParetoFrontier::by_case().build(),
            ReflectiveMutation::new("unused".to_owned()),
        );
        let selected = if mutate_answer { "answer" } else { "search" };
        let untouched = if mutate_answer { "search" } else { "answer" };
        let untouched_before = artifact.0.get(untouched).cloned();

        let change = gepa
            .change_part(&artifact, selected.to_owned(), edit.clone())
            .unwrap();
        let changed = artifact.apply_change(&change).unwrap();

        prop_assert_eq!(artifact.0.get(selected), Some(if mutate_answer { &answer } else { &search }));
        prop_assert_eq!(changed.0.get(selected), Some(&edit));
        prop_assert_eq!(changed.0.get(untouched), untouched_before.as_ref());
    }
}

#[test]
fn gepa_candidate_selector_is_population_backed() {
    let mut graph = RunGraph::<SmokeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let mut ctx = RunContext::new(&mut graph, &mut budget);
    let seed = ctx
        .insert_seed(PartMapArtifact(BTreeMap::new()), 0)
        .unwrap();
    let mut frontier = ParetoFrontier::by_case().build();
    frontier.observe_casewise_scalar(
        seed,
        leaven_kernel::AssessmentId::new(),
        &leaven_evidence::CasewiseEvidence::new(vec![leaven_evidence::CaseOutcome::new(
            leaven_kernel::CaseId::new(0),
            leaven_evidence::ScalarEvidence::new(1.0).unwrap(),
        )]),
    );
    let mut gepa = Gepa::new(
        PartMapSurface,
        frontier,
        ReflectiveMutation::new("unused".to_owned()),
    );

    assert_eq!(gepa.select_candidate(ctx.graph()), Some(seed));
}

#[test]
fn gepa_gate_policies_preserve_score_admission_laws() {
    let mut strict = StrictImprovement;
    let mut equal = ImprovementOrEqual;
    let mut no_regression = NoRegression;

    assert_eq!(strict.decide(1.0, 2.0), GateDecision::Accept);
    assert_eq!(strict.decide(1.0, 1.0), GateDecision::Reject);
    assert!(GateDecision::Accept.is_accept());
    assert!(!GateDecision::Reject.is_accept());
    assert_eq!(equal.decide(1.0, 1.0), GateDecision::Accept);
    assert_eq!(equal.decide(2.0, 1.0), GateDecision::Reject);
    assert_eq!(no_regression.decide(3.0, 3.0), GateDecision::Accept);
}

#[test]
fn gepa_explicit_strategies_are_owned_by_optimizer() {
    let mut graph = RunGraph::<SmokeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let mut ctx = RunContext::new(&mut graph, &mut budget);
    let seed = ctx
        .insert_seed(
            PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())])),
            0,
        )
        .unwrap();
    let mut frontier = ParetoFrontier::by_case().build();
    frontier.observe_casewise_scalar(
        seed,
        leaven_kernel::AssessmentId::new(),
        &leaven_evidence::CasewiseEvidence::new(vec![leaven_evidence::CaseOutcome::new(
            leaven_kernel::CaseId::new(0),
            leaven_evidence::ScalarEvidence::new(1.0).unwrap(),
        )]),
    );
    let mut gepa = Gepa::<
        PartMapSurface,
        ParetoFrontier,
        ReflectiveMutation<String>,
        SelectBestCandidate,
        leaven_gepa::RoundRobinPart,
        ImprovementOrEqual,
    >::with_strategies(
        PartMapSurface,
        frontier,
        ReflectiveMutation::new("unused".to_owned()),
        SelectBestCandidate,
        leaven_gepa::RoundRobinPart::new(),
        ImprovementOrEqual,
    );

    assert_eq!(gepa.select_candidate(ctx.graph()), Some(seed));
    assert_eq!(gepa.population().best(), Some(seed));
    assert_eq!(gepa.population_mut().best(), Some(seed));
    assert_eq!(gepa.gate_mut().decide(1.0, 1.0), GateDecision::Accept);
    assert!(matches!(
        gepa.select_part(&PartMapArtifact(BTreeMap::new())),
        Err(SurfaceError::Message(message))
            if message == "round-robin selector found no surface parts"
    ));
}

#[test]
fn gepa_selectors_delegate_to_population_best_candidate() {
    let mut graph = RunGraph::<SmokeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let ctx = RunContext::new(&mut graph, &mut budget);
    let candidate = leaven_kernel::CandidateId::new();
    let right = leaven_kernel::CandidateId::new();
    let mut keep_best = KeepBest::new();
    keep_best.observe(
        candidate,
        leaven_kernel::AssessmentId::new(),
        leaven_evidence::ScalarEvidence::new(1.0).unwrap(),
    );
    let mut tournament = TournamentPopulation::default();
    tournament.observe_pairwise(
        candidate,
        right,
        leaven_kernel::AssessmentId::new(),
        &leaven_evidence::PairwiseJudgmentEvidence::new(leaven_evidence::PairwiseJudgment::Left),
    );
    let empty_frontier = ParetoFrontier::by_case().build();
    let mut best = SelectBestCandidate;
    let mut weighted = ParetoFrequencyWeighted;

    assert_eq!(best.select(&keep_best, ctx.graph()), Some(candidate));
    assert_eq!(best.select(&tournament, ctx.graph()), Some(candidate));
    assert_eq!(weighted.select(&empty_frontier, ctx.graph()), None);
}

#[test]
fn hidden_validation_partitions_are_not_visible_to_gepa_proposers() {
    block_on(async {
        let mut graph = RunGraph::<SmokeProblem>::new(RunId::new());
        let mut budget = BudgetLedger::new(Budget::unlimited());
        let store = InlineEvidenceStore::<SmokeEvidence>::new("inline");
        let validation = leaven_core::PartitionId::from("VALIDATION");
        let case_set = CaseSet::new(vec![()])
            .with_partition(validation.clone(), vec![leaven_kernel::CaseId::new(0)]);
        let candidate = {
            let mut ctx = RunContext::<SmokeProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(
                PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())])),
                0,
            )
            .unwrap()
        };
        let assessment = {
            let evaluator = VisibilityEvaluator;
            let mut ctx = RunContext::<SmokeProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
            ctx.evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::Partition(validation.clone()),
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::Validation,
                },
            )
            .await
            .unwrap()
            .assessment_ids[0]
        };
        let proposer = HiddenPartitionInspectingProposer {
            hidden: validation.clone(),
            assessment,
            candidate,
        };
        let mut ctx = RunContext::<SmokeProblem>::new(&mut graph, &mut budget)
            .with_trust_policy(TrustPolicy::default().hide_from_proposers([validation]));

        let report = ctx.propose(&proposer, ()).await.unwrap();

        assert!(report.proposal_ids.is_empty());
        assert_eq!(report.cost, Cost::zero());
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PartMapArtifact(BTreeMap<String, String>);

#[derive(Clone, Debug, Eq, PartialEq)]
struct PartMapChange {
    part: String,
    value: String,
}

#[derive(Debug)]
struct PartMapError;

impl std::fmt::Display for PartMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("part map error")
    }
}

impl std::error::Error for PartMapError {}

impl Artifact for PartMapArtifact {
    type Change = PartMapChange;
    type ApplyError = PartMapError;

    fn identity(&self) -> ArtifactIdentity {
        let bytes = self
            .0
            .iter()
            .flat_map(|(key, value)| [key.as_bytes(), value.as_bytes()].concat())
            .collect::<Vec<_>>();
        ArtifactIdentity::Content(content_id(&bytes))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        let mut next = self.0.clone();
        if !next.contains_key(&change.part) {
            return Err(PartMapError);
        }
        next.insert(change.part.clone(), change.value.clone());
        Ok(Self(next))
    }
}

struct SmokeProblem;

impl OptimizationProblem for SmokeProblem {
    type Artifact = PartMapArtifact;
    type Case = ();
    type Evidence = SmokeEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug)]
struct SmokeEvidence;

impl Evidence for SmokeEvidence {}

struct VisibilityEvaluator;

impl Evaluator<SmokeProblem> for VisibilityEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([7; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, SmokeProblem>,
    ) -> Result<Metered<Vec<Assessment<SmokeProblem>>>, EvaluationError> {
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        Ok(Metered::new(
            candidates
                .into_iter()
                .map(|candidate| Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::EvaluationSet(leaven_kernel::EvaluationSetId::new()),
                    evidence: SmokeEvidence,
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                })
                .collect(),
            Cost::metric_calls(1),
        ))
    }
}

struct HiddenPartitionInspectingProposer {
    hidden: leaven_core::PartitionId,
    assessment: leaven_kernel::AssessmentId,
    candidate: leaven_kernel::CandidateId,
}

impl Proposer<SmokeProblem> for HiddenPartitionInspectingProposer {
    type Request = ();

    fn id(&self) -> ProposerId {
        ProposerId::from("gepa-reflection")
    }

    async fn propose(
        &self,
        _request: Self::Request,
        ctx: ProposalContext<'_, SmokeProblem>,
    ) -> Result<Metered<ProposalBatch<SmokeProblem>>, ProposalError> {
        assert!(ctx.read_scope().hidden_partitions.contains(&self.hidden));
        assert!(ctx.graph().assessment(self.assessment).is_none());
        assert!(ctx.graph().assessments(self.candidate).is_empty());
        Ok(Metered::new(
            ProposalBatch {
                proposals: Vec::new(),
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        ))
    }
}

#[derive(Clone, Debug)]
struct PartMapSurface;

impl EditSurface<PartMapArtifact> for PartMapSurface {
    type PartId = String;
    type Address = PartAddress;
    type View<'a> = &'a str;
    type Edit = String;

    fn fingerprint(&self) -> SurfaceFingerprint {
        SurfaceFingerprint(leaven_kernel::Fingerprint::from_bytes([3; 32]))
    }

    fn parts<'a>(
        &self,
        artifact: &'a PartMapArtifact,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        Ok(artifact
            .0
            .iter()
            .map(|(id, value)| Part {
                id: id.clone(),
                address: PartAddress(id.clone()),
                view: value.as_str(),
            })
            .collect())
    }

    fn change_part(
        &self,
        artifact: &PartMapArtifact,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<<PartMapArtifact as Artifact>::Change, SurfaceError> {
        if artifact.0.contains_key(&id) {
            Ok(PartMapChange {
                part: id,
                value: edit,
            })
        } else {
            Err(SurfaceError::UnknownPart)
        }
    }
}

fn content_id(bytes: &[u8]) -> ContentId {
    let mut id = [0; ContentId::BYTES];
    let len = bytes.len().min(ContentId::BYTES);
    id[..len].copy_from_slice(&bytes[..len]);
    ContentId::from_bytes(id)
}
