use std::collections::BTreeMap;

use futures::executor::block_on;
use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, Evidence, OptimizationProblem,
    ProposalBatch, ProposalBatchSemantics, ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_engine::{
    BudgetLedger, CachePolicy, CaseSet, CheckpointContext, CheckpointableOptimizer, Engine,
    EvaluationContext, EvaluationError, Evaluator, PrivateStatePolicy, ProposalContext,
    ProposalError, Proposer, RestoreContext, RunContext, RunGraph, TrustPolicy,
};
use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence, ScoredFeedbackEvidence};
use leaven_gepa::{
    CandidateSelector, CheckpointCandidateSelector, CheckpointGate, CheckpointPopulation,
    FixedSurfaceEdit, Gate, GateDecision, Gepa, ImprovementOrEqual, NoRegression,
    ParetoFrequencyWeighted, SelectBestCandidate, StrictImprovement, SurfaceProposer,
    optimizer::GepaPopulation,
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
        FixedSurfaceEdit::new("unused".to_owned()),
    );
    let mut proposer = FixedSurfaceEdit::new("improved".to_owned());

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
            FixedSurfaceEdit::new("unused".to_owned()),
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
        FixedSurfaceEdit::new("unused".to_owned()),
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
    CheckpointGate::checkpoint_state(&equal);
    CheckpointGate::restore_state(&mut equal, ());
    CheckpointGate::checkpoint_state(&no_regression);
    CheckpointGate::restore_state(&mut no_regression, ());
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
        FixedSurfaceEdit<String>,
        SelectBestCandidate,
        leaven_gepa::RoundRobinPart,
        ImprovementOrEqual,
    >::with_strategies(
        PartMapSurface,
        frontier,
        FixedSurfaceEdit::new("unused".to_owned()),
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
fn gepa_checkpoint_state_restores_loop_and_selector_cursor() {
    let artifact = PartMapArtifact(BTreeMap::from([
        ("answer".to_owned(), "draft".to_owned()),
        ("search".to_owned(), "query".to_owned()),
    ]));
    let mut graph = RunGraph::<SmokeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let mut ctx = RunContext::new(&mut graph, &mut budget);
    ctx.insert_seed(artifact.clone(), 0).unwrap();
    let mut gepa = Gepa::new(
        PartMapSurface,
        ParetoFrontier::by_case().build(),
        FixedSurfaceEdit::new("unused".to_owned()),
    );

    assert_eq!(gepa.select_part(&artifact).unwrap(), "answer");
    let state = gepa
        .checkpoint_state(CheckpointContext::new(ctx.graph()))
        .unwrap();
    let policy = <Gepa<PartMapSurface, ParetoFrontier, FixedSurfaceEdit<String>> as CheckpointableOptimizer<
        SmokeProblem,
    >>::private_state_policy(&gepa);
    assert!(matches!(
        policy,
        PrivateStatePolicy::ExplicitSnapshot { .. }
    ));

    let mut restored = Gepa::new(
        PartMapSurface,
        ParetoFrontier::by_case().build(),
        FixedSurfaceEdit::new("unused".to_owned()),
    );
    restored
        .restore_state(state, RestoreContext::new(ctx.graph()))
        .unwrap();

    assert_eq!(restored.select_part(&artifact).unwrap(), "search");
}

#[test]
fn gepa_checkpoint_state_restores_population_frontier_membership() {
    let artifact = PartMapArtifact(BTreeMap::from([
        ("answer".to_owned(), "draft".to_owned()),
        ("search".to_owned(), "query".to_owned()),
    ]));
    let mut graph = RunGraph::<SmokeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let mut ctx = RunContext::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(artifact, 0).unwrap();
    let mut gepa = Gepa::new(
        PartMapSurface,
        ParetoFrontier::by_case()
            .partition_filter(std::collections::BTreeSet::from(["TRAIN".into()]))
            .build(),
        FixedSurfaceEdit::new("unused".to_owned()),
    );
    let evidence = CasewiseEvidence::new(vec![CaseOutcome::new(
        leaven_kernel::CaseId::new(0),
        ScalarEvidence::new(1.0).unwrap(),
    )]);
    gepa.population_mut().observe_partitioned_casewise_scalar(
        &"TRAIN".into(),
        seed,
        leaven_kernel::AssessmentId::new(),
        &evidence,
    );

    assert_eq!(gepa.select_candidate(ctx.graph()), Some(seed));
    let state = gepa
        .checkpoint_state(CheckpointContext::new(ctx.graph()))
        .unwrap();
    let mut restored = Gepa::new(
        PartMapSurface,
        ParetoFrontier::by_case()
            .partition_filter(std::collections::BTreeSet::from(["TRAIN".into()]))
            .build(),
        FixedSurfaceEdit::new("unused".to_owned()),
    );
    restored
        .restore_state(state, RestoreContext::new(ctx.graph()))
        .unwrap();

    assert_eq!(restored.population().best(), Some(seed));
    assert_eq!(restored.select_candidate(ctx.graph()), Some(seed));
}

#[test]
fn gepa_checkpoint_restore_rejects_missing_best_and_observed_candidates() {
    let artifact = PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())]));
    let mut graph = RunGraph::<SmokeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let mut ctx = RunContext::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(artifact, 0).unwrap();
    let gepa = Gepa::new(
        PartMapSurface,
        ParetoFrontier::by_case().build(),
        FixedSurfaceEdit::new("unused".to_owned()),
    );
    let state = gepa
        .checkpoint_state(CheckpointContext::new(ctx.graph()))
        .unwrap();
    let mut missing_best = serde_json::to_value(&state).unwrap();
    missing_best["best"] = serde_json::to_value(leaven_kernel::CandidateId::new()).unwrap();
    let missing_best_state = serde_json::from_value(missing_best).unwrap();
    let mut restored = Gepa::new(
        PartMapSurface,
        ParetoFrontier::by_case().build(),
        FixedSurfaceEdit::new("unused".to_owned()),
    );

    let error = restored
        .restore_state(missing_best_state, RestoreContext::new(ctx.graph()))
        .unwrap_err();

    assert!(error.to_string().contains("best candidate"));

    let mut missing_observed = serde_json::to_value(&state).unwrap();
    missing_observed["best"] = serde_json::to_value(seed).unwrap();
    missing_observed["observed"] = serde_json::json!([leaven_kernel::CandidateId::new()]);
    let observed_state = serde_json::from_value(missing_observed).unwrap();
    let error = restored
        .restore_state(observed_state, RestoreContext::new(ctx.graph()))
        .unwrap_err();

    assert!(error.to_string().contains("observed candidate"));
}

#[test]
fn gepa_checkpoint_population_round_trips_keep_best_state() {
    let candidate = leaven_kernel::CandidateId::new();
    let mut population = KeepBest::new();
    population.observe(
        candidate,
        leaven_kernel::AssessmentId::new(),
        ScalarEvidence::new(1.0).unwrap(),
    );

    let state = CheckpointPopulation::checkpoint_state(&population);
    let mut restored = KeepBest::new();
    CheckpointPopulation::restore_state(&mut restored, state);

    assert_eq!(restored.best(), Some(candidate));
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
    CheckpointCandidateSelector::checkpoint_state(&best);
    CheckpointCandidateSelector::restore_state(&mut best, ());
    CheckpointCandidateSelector::checkpoint_state(&weighted);
    CheckpointCandidateSelector::restore_state(&mut weighted, ());
}

#[test]
fn gepa_score_evidence_projects_feedback_scores_to_scalar_casewise() {
    let scored = CasewiseEvidence::new(vec![
        CaseOutcome::new(
            leaven_kernel::CaseId::new(0),
            ScoredFeedbackEvidence::new(
                ScalarEvidence::new(1.0).unwrap(),
                "correct".to_owned(),
                vec!["trace".to_owned()],
            ),
        ),
        CaseOutcome::new(
            leaven_kernel::CaseId::new(1),
            ScoredFeedbackEvidence::new(
                ScalarEvidence::new(0.5).unwrap(),
                "partial".to_owned(),
                vec!["trace".to_owned()],
            ),
        ),
    ]);
    let scalar = leaven_gepa::GepaScoreEvidence::scalar_casewise(&scored);

    assert_eq!(
        leaven_gepa::GepaScoreEvidence::average_score(&scored),
        Some(0.75)
    );
    assert!((scalar.outcomes()[0].evidence().score() - 1.0).abs() < f64::EPSILON);
    assert!((scalar.outcomes()[1].evidence().score() - 0.5).abs() < f64::EPSILON);
    assert_eq!(
        leaven_gepa::GepaScoreEvidence::average_score(&CasewiseEvidence::<ScalarEvidence>::new(
            Vec::new()
        )),
        None
    );
}

#[test]
fn keep_best_gepa_population_ignores_empty_casewise_and_averages_scores() {
    let candidate = leaven_kernel::CandidateId::new();
    let assessment = leaven_kernel::AssessmentId::new();
    let empty = CasewiseEvidence::<ScalarEvidence>::new(Vec::new());
    let mut keep_best = KeepBest::new();

    assert_eq!(GepaPopulation::id(&keep_best), keep_best.id());
    let ignored = GepaPopulation::observe_gepa(
        &mut keep_best,
        Some(&leaven_core::PartitionId::from("TRAIN")),
        candidate,
        assessment,
        &empty,
    );

    assert!(matches!(
        &ignored[..],
        [leaven_engine::PopulationEvent::Ignored { candidate: observed, .. }]
            if *observed == candidate
    ));
    assert_eq!(GepaPopulation::best(&keep_best), None);

    let scored = CasewiseEvidence::new(vec![
        CaseOutcome::new(
            leaven_kernel::CaseId::new(0),
            ScalarEvidence::new(0.25).unwrap(),
        ),
        CaseOutcome::new(
            leaven_kernel::CaseId::new(1),
            ScalarEvidence::new(0.75).unwrap(),
        ),
    ]);
    let events = GepaPopulation::observe_gepa(&mut keep_best, None, candidate, assessment, &scored);

    assert!(!events.is_empty());
    assert_eq!(GepaPopulation::best(&keep_best), Some(candidate));

    let mut frontier = ParetoFrontier::by_case().build();
    let frontier_events =
        GepaPopulation::observe_gepa(&mut frontier, None, candidate, assessment, &scored);
    assert!(!frontier_events.is_empty());
}

#[test]
fn gepa_builder_default_reflector_path_uses_pareto_frontier_defaults() {
    let gepa = Gepa::builder()
        .surface(PartMapSurface)
        .reflector(FixedSurfaceEdit::new("improved".to_owned()))
        .max_iterations(2);

    assert_eq!(gepa.population().best(), None);
}

#[test]
fn gepa_run_reports_missing_seed_before_evaluation() {
    block_on(async {
        let case_set = train_case_set();
        let store = InlineEvidenceStore::<SmokeEvidence>::new("inline");
        let mut engine = Engine::<SmokeProblem>::builder()
            .evaluator(VisibilityEvaluator)
            .build();
        let mut gepa = Gepa::new(
            PartMapSurface,
            ParetoFrontier::by_case().build(),
            FixedSurfaceEdit::new("unused".to_owned()),
        );

        let error = engine.run(&mut gepa, &case_set, &store).await.unwrap_err();

        assert!(error.to_string().contains("at least one seed candidate"));
    });
}

#[test]
fn gepa_run_reports_empty_casewise_scores() {
    block_on(async {
        let case_set = train_case_set();
        let store = InlineEvidenceStore::<SmokeEvidence>::new("inline");
        let mut engine = Engine::<SmokeProblem>::builder()
            .evaluator(VisibilityEvaluator)
            .build();
        engine
            .insert_seed(
                PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())])),
                0,
            )
            .unwrap();
        let mut gepa = Gepa::new(
            PartMapSurface,
            ParetoFrontier::by_case().build(),
            FixedSurfaceEdit::new("unused".to_owned()),
        );

        let error = engine.run(&mut gepa, &case_set, &store).await.unwrap_err();

        assert!(error.to_string().contains("casewise scores"));
    });
}

#[test]
fn gepa_zero_iterations_finishes_without_best_candidate() {
    block_on(async {
        let case_set = train_case_set();
        let store = InlineEvidenceStore::<SmokeEvidence>::new("inline");
        let mut engine = Engine::<SmokeProblem>::builder()
            .evaluator(VisibilityEvaluator)
            .build();
        engine
            .insert_seed(
                PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())])),
                0,
            )
            .unwrap();
        let mut gepa = Gepa::new(
            PartMapSurface,
            ParetoFrontier::by_case().build(),
            FixedSurfaceEdit::new("unused".to_owned()),
        )
        .max_iterations(0);

        let run = engine.run(&mut gepa, &case_set, &store).await.unwrap();

        assert_eq!(run.best, None);
    });
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

fn train_case_set() -> CaseSet<()> {
    CaseSet::new(vec![()]).with_partition(
        leaven_core::PartitionId::from("TRAIN"),
        vec![leaven_kernel::CaseId::new(0)],
    )
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

impl leaven_gepa::GepaScoreEvidence for SmokeEvidence {
    fn scalar_casewise(&self) -> CasewiseEvidence<ScalarEvidence> {
        CasewiseEvidence::new(Vec::new())
    }
}

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
