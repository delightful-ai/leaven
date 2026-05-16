use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use futures::executor::block_on;
use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, Evidence, OptimizationProblem,
    ProposalBatch, ProposalBatchSemantics, ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_engine::{
    BudgetLedger, CachePolicy, CaseSet, CheckpointContext, CheckpointableOptimizer, Engine,
    EvaluationContext, EvaluationError, Evaluator, GraphSnapshotRef, Optimizer,
    OptimizerStateReader, PrivateStatePolicy, ProposalContext, ProposalError, Proposer,
    RestoreContext, RunCheckpoint, RunContext, RunGraph, RunPersistenceError, StateFormat,
    TrustPolicy,
};
use leaven_evidence::{
    CaseAssessmentEvidence, CaseOutcome, CasewiseEvidence, OutputRecord, ScalarEvidence,
};
use leaven_gepa::{
    CandidateSelector, CheckpointCandidateSelector, CheckpointGate, CheckpointPopulation,
    FixedSurfaceEdit, FullValidation, Gate, GateDecision, Gepa, GepaReflector, ImprovementOrEqual,
    NoRegression, ParetoFrequencyWeighted, ReflectRequest, ReflectiveCaseInput,
    ReflectiveDatasetBuilder, ReflectiveExample, SelectBestCandidate, StrictImprovement,
    SurfaceProposer,
    optimizer::GepaPopulation,
    validation::{
        BatchSampler, CheckpointBatchSampler, CheckpointValidationPolicy, EpochShuffled,
        MinibatchThenValidation, ValidationPolicy,
    },
};
use leaven_kernel::{
    BlobRef, Budget, BudgetSnapshot, ContentId, Cost, EvaluatorId, Fingerprint, MetadataBag,
    Metered, ProposerId, RunId,
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

#[test]
fn public_builder_supports_explicit_population_before_reflector() {
    let mut gepa = Gepa::builder()
        .surface(PartMapSurface)
        .population(ParetoFrontier::by_case().build())
        .reflector(FixedSurfaceEdit::new("builder-edit".to_owned()));
    let artifact = PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())]));

    let part = gepa.select_part(&artifact).unwrap();
    let changed = artifact
        .apply_change(
            &gepa
                .change_part(&artifact, part, "builder-edit".to_owned())
                .unwrap(),
        )
        .unwrap();

    assert_eq!(changed.0.get("answer").unwrap(), "builder-edit");
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
fn epoch_shuffled_samples_train_with_seed_and_restores_cursor() {
    let train = leaven_core::PartitionId::from("TRAIN");
    let mut sampler = EpochShuffled::new(2).with_seed(41);

    assert!(matches!(
        sampler.sample_train(&train),
        EvaluationSet::Sample { of, n: 2, seed: 41 }
            if matches!(*of, EvaluationSet::Partition(ref partition) if partition == &train)
    ));
    assert!(matches!(
        sampler.sample_train(&train),
        EvaluationSet::Sample { of, n: 2, seed: 42 }
            if matches!(*of, EvaluationSet::Partition(ref partition) if partition == &train)
    ));

    let state = CheckpointBatchSampler::checkpoint_state(&sampler);
    let mut restored = EpochShuffled::default();
    CheckpointBatchSampler::restore_state(&mut restored, state);

    assert!(matches!(
        restored.sample_train(&train),
        EvaluationSet::Sample { of, n: 2, seed: 43 }
            if matches!(*of, EvaluationSet::Partition(ref partition) if partition == &train)
    ));
}

#[test]
fn validation_policies_expose_held_out_and_default_skip_behaviors() {
    let accepted = leaven_kernel::CandidateId::new();
    let mut full = FullValidation;
    let mut default = MinibatchThenValidation;

    assert!(matches!(
        full.validation_set(accepted),
        Some(EvaluationSet::Partition(partition)) if partition == leaven_core::PartitionId::from("VALIDATION")
    ));
    assert!(default.validation_set(accepted).is_none());
    CheckpointValidationPolicy::checkpoint_state(&full);
    CheckpointValidationPolicy::restore_state(&mut full, ());
    CheckpointValidationPolicy::checkpoint_state(&default);
    CheckpointValidationPolicy::restore_state(&mut default, ());
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
fn gepa_checkpoint_schema_changed_when_batch_sampler_state_was_added() {
    let gepa = smoke_gepa(FixedSurfaceEdit::new("unused".to_owned()));

    let policy = <SmokeGepa as CheckpointableOptimizer<SmokeProblem>>::private_state_policy(&gepa);

    assert!(matches!(
        policy,
        PrivateStatePolicy::ExplicitSnapshot { schema, .. }
            if schema != Fingerprint::from_bytes([7; 32])
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
    let mut gepa = smoke_gepa(FixedSurfaceEdit::new("unused".to_owned()));

    assert_eq!(gepa.select_part(&artifact).unwrap(), "answer");
    let state = gepa
        .checkpoint_state(CheckpointContext::new(ctx.graph()))
        .unwrap();
    let policy = <SmokeGepa as CheckpointableOptimizer<SmokeProblem>>::private_state_policy(&gepa);
    assert!(matches!(
        policy,
        PrivateStatePolicy::ExplicitSnapshot { .. }
    ));

    let mut restored = smoke_gepa(FixedSurfaceEdit::new("unused".to_owned()));
    restored
        .restore_state(state, RestoreContext::new(ctx.graph()))
        .unwrap();

    assert_eq!(restored.select_part(&artifact).unwrap(), "search");
}

#[test]
fn gepa_restore_checkpoint_state_uses_engine_resume_contract() {
    let artifact = PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())]));
    let mut graph = RunGraph::<SmokeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let mut ctx = RunContext::new(&mut graph, &mut budget);
    ctx.insert_seed(artifact, 0).unwrap();
    let mut gepa = smoke_gepa(FixedSurfaceEdit::new("unused".to_owned()));
    let checkpoint = RunCheckpoint::new(
        RunId::new(),
        leaven_kernel::now(),
        GraphSnapshotRef {
            schema: Fingerprint::from_bytes([1; 32]),
            format: StateFormat::Json,
            bytes: BlobRef {
                store: "test".to_owned(),
                key: "graph".to_owned(),
            },
        },
        BudgetSnapshot::default(),
    );

    let error = Optimizer::<SmokeProblem>::restore_checkpoint_state(
        &mut gepa,
        &checkpoint,
        &MissingOptimizerStateReader,
        RestoreContext::new(ctx.graph()),
    )
    .unwrap_err();

    assert!(format!("{error:?}").contains("does not contain optimizer private state"));
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
    let mut gepa = smoke_gepa_with_population(
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
    let mut restored = smoke_gepa_with_population(
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

struct MissingOptimizerStateReader;

impl OptimizerStateReader for MissingOptimizerStateReader {
    fn load_optimizer_state<T>(
        &self,
        _checkpoint: &RunCheckpoint,
        _optimizer: Fingerprint,
        _schema: Fingerprint,
    ) -> Result<Option<T>, RunPersistenceError>
    where
        T: serde::de::DeserializeOwned,
    {
        Ok(None)
    }
}

#[test]
fn gepa_checkpoint_restore_rejects_missing_best_and_observed_candidates() {
    let artifact = PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())]));
    let mut graph = RunGraph::<SmokeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let mut ctx = RunContext::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(artifact, 0).unwrap();
    let gepa = smoke_gepa(FixedSurfaceEdit::new("unused".to_owned()));
    let state = gepa
        .checkpoint_state(CheckpointContext::new(ctx.graph()))
        .unwrap();
    let mut missing_best = serde_json::to_value(&state).unwrap();
    missing_best["best"] = serde_json::to_value(leaven_kernel::CandidateId::new()).unwrap();
    let missing_best_state = serde_json::from_value(missing_best).unwrap();
    let mut restored = smoke_gepa(FixedSurfaceEdit::new("unused".to_owned()));

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
fn gepa_candidate_selector_slot_uses_upstream_language() {
    let mut graph = RunGraph::<SmokeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let ctx = RunContext::new(&mut graph, &mut budget);
    let candidate = leaven_kernel::CandidateId::new();
    let mut keep_best = KeepBest::new();
    keep_best.observe(
        candidate,
        leaven_kernel::AssessmentId::new(),
        leaven_evidence::ScalarEvidence::new(1.0).unwrap(),
    );
    let mut weighted = ParetoFrequencyWeighted;

    assert_eq!(weighted.select(&keep_best, ctx.graph()), Some(candidate));
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
fn gepa_case_evidence_projects_feedback_scores_to_scalar_rows() {
    let scored = CaseAssessmentEvidence::new(
        ScalarEvidence::new(1.0).unwrap(),
        OutputRecord::inline("correct output"),
        "correct".to_owned(),
    );
    let scalar = ScalarEvidence::new(0.5).unwrap();

    assert!(
        (leaven_gepa::GepaCaseEvidence::scalar_score(&scored)
            .unwrap()
            .score()
            - 1.0)
            .abs()
            < f64::EPSILON
    );
    assert!(
        (leaven_gepa::GepaCaseEvidence::scalar_score(&scalar)
            .unwrap()
            .score()
            - 0.5)
            .abs()
            < f64::EPSILON
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
        &[assessment],
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
    let events =
        GepaPopulation::observe_gepa(&mut keep_best, None, candidate, &[assessment], &scored);

    assert!(!events.is_empty());
    assert_eq!(GepaPopulation::best(&keep_best), Some(candidate));

    let mut frontier = ParetoFrontier::by_case().build();
    let frontier_events =
        GepaPopulation::observe_gepa(&mut frontier, None, candidate, &[assessment], &scored);
    assert!(!frontier_events.is_empty());
}

#[test]
fn gepa_default_sampler_uses_train_minibatches_without_validation_or_test_cases() {
    block_on(async {
        let seen_sets = Arc::new(Mutex::new(Vec::new()));
        let case_set = CaseSet::new(vec![(), (), (), (), (), ()])
            .with_partition(
                leaven_core::PartitionId::from("TRAIN"),
                vec![
                    leaven_kernel::CaseId::new(0),
                    leaven_kernel::CaseId::new(1),
                    leaven_kernel::CaseId::new(2),
                    leaven_kernel::CaseId::new(3),
                ],
            )
            .with_partition(
                leaven_core::PartitionId::from("VALIDATION"),
                vec![leaven_kernel::CaseId::new(4)],
            )
            .with_partition(
                leaven_core::PartitionId::from("TEST"),
                vec![leaven_kernel::CaseId::new(5)],
            );
        let store = InlineEvidenceStore::<ScalarEvidence>::new("inline");
        let mut engine = Engine::<SamplingProblem>::builder()
            .evaluator(RecordingCaseSetEvaluator {
                seen_sets: seen_sets.clone(),
            })
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
            FixedSurfaceEdit::new("improved".to_owned()),
        )
        .reflective_dataset(NoReflectiveExamples)
        .max_iterations(1);

        engine.run(&mut gepa, &case_set, &store).await.unwrap();

        let seen_sets = seen_sets.lock().expect("seen sets lock").clone();
        assert_eq!(seen_sets.len(), 2);
        for case_ids in seen_sets {
            assert_eq!(case_ids.len(), 3);
            assert!(
                case_ids
                    .iter()
                    .all(|case| *case < leaven_kernel::CaseId::new(4))
            );
        }
    });
}

#[test]
fn gepa_candidate_history_tracks_seed_and_accepted_children_by_assessment() {
    block_on(async {
        let case_set = train_case_set();
        let store = InlineEvidenceStore::<ScalarEvidence>::new("inline");
        let mut engine = Engine::<SamplingProblem>::builder()
            .evaluator(PrefixImprovementEvaluator)
            .build();
        let seed = engine
            .insert_seed(
                PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())])),
                0,
            )
            .unwrap();
        let mut gepa = Gepa::new(
            PartMapSurface,
            ParetoFrontier::by_case().build(),
            FixedSurfaceEdit::new("improved".to_owned()),
        )
        .reflective_dataset(NoReflectiveExamples)
        .max_iterations(1);

        engine.run(&mut gepa, &case_set, &store).await.unwrap();

        let child = engine.view().candidate_tree().children(seed)[0];
        let history = gepa.candidate_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].candidate(), seed);
        assert!((history[0].score() - 0.0).abs() < f64::EPSILON);
        assert_eq!(history[1].candidate(), child);
        assert!((history[1].score() - 1.0).abs() < f64::EPSILON);
        assert_eq!(history[0].assessments().len(), 1);
        assert_eq!(history[1].assessments().len(), 1);
        assert!(
            history[0]
                .assessments()
                .iter()
                .all(|assessment| engine.view().assessment(*assessment).is_some())
        );
        assert!(
            history[1]
                .assessments()
                .iter()
                .all(|assessment| engine.view().assessment(*assessment).is_some())
        );
    });
}

#[test]
fn gepa_reflective_dataset_default_projects_scalar_examples_with_case_input() {
    block_on(async {
        let case_set = CaseSet::new(vec!["input alpha"]).with_partition(
            leaven_core::PartitionId::from("TRAIN"),
            vec![leaven_kernel::CaseId::new(0)],
        );
        let store = InlineEvidenceStore::<ScalarEvidence>::new("inline");
        let mut graph = RunGraph::<DisplayScalarProblem>::new(RunId::new());
        let mut budget = BudgetLedger::new(Budget::unlimited());
        let candidate = {
            let mut ctx = RunContext::<DisplayScalarProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(
                PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())])),
                0,
            )
            .unwrap()
        };
        let assessments = {
            let mut ctx = RunContext::<DisplayScalarProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
            ctx.evaluate_with(&ScalarCaseEvaluator, independent_train_request(candidate))
                .await
                .unwrap()
                .assessment_ids
        };

        let mut ctx = RunContext::<DisplayScalarProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_evidence_store(&store);
        let examples = ReflectiveDatasetBuilder::<DisplayScalarProblem, PartMapSurface>::build(
            &leaven_gepa::GepaReflectiveDataset,
            &mut ctx,
            candidate,
            &assessments,
            &"answer".to_owned(),
        )
        .await
        .unwrap();

        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].input, "input alpha");
        assert_eq!(examples[0].score, Some(0.5));
        assert!(examples[0].output.is_none());
        assert!(examples[0].feedback.is_empty());
        assert!(
            examples[0]
                .source_refs
                .contains(&leaven_core::InfoRef::Assessment(assessments[0]))
        );
    });
}

#[test]
fn gepa_reflective_dataset_uses_target_safe_case_projection() {
    block_on(async {
        let case_set = CaseSet::new(vec![HiddenTargetCase {
            safe_input: "visible problem statement",
            hidden_target: "SECRET_TARGET_42",
            hidden_metadata: "SECRET_SOURCE_ROW",
        }])
        .with_partition(
            leaven_core::PartitionId::from("TRAIN"),
            vec![leaven_kernel::CaseId::new(0)],
        );
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut graph = RunGraph::<HiddenTargetProblem>::new(RunId::new());
        let mut budget = BudgetLedger::new(Budget::unlimited());
        let candidate = {
            let mut ctx = RunContext::<HiddenTargetProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(
                PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())])),
                0,
            )
            .unwrap()
        };
        let assessments = {
            let mut ctx = RunContext::<HiddenTargetProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
            ctx.evaluate_with(&HiddenTargetEvaluator, independent_train_request(candidate))
                .await
                .unwrap()
                .assessment_ids
        };

        let mut ctx = RunContext::<HiddenTargetProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_evidence_store(&store);
        let examples = ReflectiveDatasetBuilder::<HiddenTargetProblem, PartMapSurface>::build(
            &leaven_gepa::GepaReflectiveDataset,
            &mut ctx,
            candidate,
            &assessments,
            &"answer".to_owned(),
        )
        .await
        .unwrap();
        let projected_examples =
            ReflectiveDatasetBuilder::<HiddenTargetProblem, PartMapSurface>::build(
                &leaven_gepa::GepaReflectiveDataset::with_case_input(|case: &HiddenTargetCase| {
                    case.safe_input.to_owned()
                }),
                &mut ctx,
                candidate,
                &assessments,
                &"answer".to_owned(),
            )
            .await
            .unwrap();

        assert_eq!(projected_examples, examples);
        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].case, Some(leaven_kernel::CaseId::new(0)));
        assert_eq!(examples[0].input, "visible problem statement");
        assert_eq!(examples[0].output.as_deref(), Some("candidate output"));
        assert_eq!(examples[0].score, Some(0.25));
        assert_eq!(examples[0].feedback, "visible scorer feedback");
        assert!(
            examples[0]
                .source_refs
                .contains(&leaven_core::InfoRef::Assessment(assessments[0]))
        );

        let serialized = serde_json::to_string(&examples).unwrap();
        assert!(!serialized.contains("SECRET_TARGET_42"));
        assert!(!serialized.contains("SECRET_SOURCE_ROW"));
        assert!(!serialized.contains("display leak"));
    });
}

#[test]
fn gepa_reflective_dataset_default_reports_missing_assessment_evidence() {
    block_on(async {
        let store = InlineEvidenceStore::<ScalarEvidence>::new("inline");
        let mut graph = RunGraph::<DisplayScalarProblem>::new(RunId::new());
        let mut budget = BudgetLedger::new(Budget::unlimited());
        let candidate = {
            let mut ctx = RunContext::<DisplayScalarProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(PartMapArtifact(BTreeMap::new()), 0)
                .unwrap()
        };
        let mut ctx = RunContext::<DisplayScalarProblem>::new(&mut graph, &mut budget)
            .with_evidence_store(&store);

        let error = ReflectiveDatasetBuilder::<DisplayScalarProblem, PartMapSurface>::build(
            &leaven_gepa::GepaReflectiveDataset,
            &mut ctx,
            candidate,
            &[leaven_kernel::AssessmentId::new()],
            &"answer".to_owned(),
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("parent assessment row"));
    });
}

#[test]
fn gepa_run_surfaces_reflective_dataset_build_failure() {
    block_on(async {
        let case_set = CaseSet::new(vec!["input alpha"]).with_partition(
            leaven_core::PartitionId::from("TRAIN"),
            vec![leaven_kernel::CaseId::new(0)],
        );
        let store = InlineEvidenceStore::<ScalarEvidence>::new("inline");
        let mut engine = Engine::<DisplayScalarProblem>::builder()
            .evaluator(ScalarCaseEvaluator)
            .build();
        engine
            .insert_seed(
                PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())])),
                0,
            )
            .unwrap();
        let failing_dataset = |_ctx: &mut RunContext<'_, DisplayScalarProblem>,
                               _parent: leaven_kernel::CandidateId,
                               _assessments: &[leaven_kernel::AssessmentId],
                               _part: &String| async move {
            Result::<Vec<ReflectiveExample>, leaven_gepa::ReflectionError>::Err(
                leaven_gepa::ReflectionError::builder("scripted dataset failure"),
            )
        };
        let mut gepa = Gepa::new(
            PartMapSurface,
            ParetoFrontier::by_case().build(),
            FixedSurfaceEdit::new("improved".to_owned()),
        )
        .reflective_dataset(failing_dataset)
        .max_iterations(1);

        let error = engine.run(&mut gepa, &case_set, &store).await.unwrap_err();

        assert!(
            error
                .to_string()
                .contains("GEPA reflective-dataset build failed")
        );
    });
}

#[test]
fn gepa_reflective_dataset_closure_builder_can_refuse() {
    block_on(async {
        let builder = |_ctx: &mut RunContext<'_, DisplayScalarProblem>,
                       _parent: leaven_kernel::CandidateId,
                       _assessments: &[leaven_kernel::AssessmentId],
                       _part: &String| async move {
            Result::<Vec<ReflectiveExample>, leaven_gepa::ReflectionError>::Err(
                leaven_gepa::ReflectionError::builder("custom dataset declined"),
            )
        };
        let mut graph = RunGraph::<DisplayScalarProblem>::new(RunId::new());
        let mut budget = BudgetLedger::new(Budget::unlimited());
        let mut ctx = RunContext::<DisplayScalarProblem>::new(&mut graph, &mut budget);

        let error = ReflectiveDatasetBuilder::<DisplayScalarProblem, PartMapSurface>::build(
            &builder,
            &mut ctx,
            leaven_kernel::CandidateId::new(),
            &[leaven_kernel::AssessmentId::new()],
            &"answer".to_owned(),
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("custom dataset declined"));
    });
}

#[test]
fn gepa_batch_sampler_builder_uses_custom_minibatches() {
    block_on(async {
        let seen_sets = Arc::new(Mutex::new(Vec::new()));
        let case_set = CaseSet::new(vec![(), (), (), ()]).with_partition(
            leaven_core::PartitionId::from("TRAIN"),
            vec![
                leaven_kernel::CaseId::new(0),
                leaven_kernel::CaseId::new(1),
                leaven_kernel::CaseId::new(2),
                leaven_kernel::CaseId::new(3),
            ],
        );
        let store = InlineEvidenceStore::<ScalarEvidence>::new("inline");
        let mut engine = Engine::<SamplingProblem>::builder()
            .evaluator(RecordingCaseSetEvaluator {
                seen_sets: seen_sets.clone(),
            })
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
            FixedSurfaceEdit::new("improved".to_owned()),
        )
        .reflective_dataset(NoReflectiveExamples)
        .batch_sampler(EpochShuffled::new(2).with_seed(7))
        .max_iterations(1);

        engine.run(&mut gepa, &case_set, &store).await.unwrap();

        let seen_sets = seen_sets.lock().expect("seen sets lock").clone();
        assert_eq!(seen_sets.len(), 2);
        assert!(seen_sets.iter().all(|case_ids| case_ids.len() == 2));
    });
}

#[test]
fn gepa_proposal_count_applies_multiple_reflections_in_one_iteration() {
    block_on(async {
        let case_set = train_case_set();
        let store = InlineEvidenceStore::<ScalarEvidence>::new("inline");
        let mut engine = Engine::<SamplingProblem>::builder()
            .evaluator(PrefixImprovementEvaluator)
            .build();
        let seed = engine
            .insert_seed(
                PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())])),
                0,
            )
            .unwrap();
        let mut gepa = Gepa::new(
            PartMapSurface,
            ParetoFrontier::by_case().build(),
            SequentialSurfaceEdits::new(["improved-a", "improved-b"]),
        )
        .reflective_dataset(NoReflectiveExamples)
        .proposal_count(2)
        .max_iterations(1);

        engine.run(&mut gepa, &case_set, &store).await.unwrap();

        let view = engine.view();
        let children = view.candidate_tree().children(seed);
        let child_artifacts = children
            .iter()
            .map(|child| {
                view.artifact(*child)
                    .unwrap()
                    .0
                    .get("answer")
                    .unwrap()
                    .clone()
            })
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 2);
        assert!(child_artifacts.contains(&"improved-a".to_owned()));
        assert!(child_artifacts.contains(&"improved-b".to_owned()));
        assert_eq!(view.proposal_batch_count(), 2);
        assert_eq!(view.assessment_count(), 3);
    });
}

#[test]
fn full_validation_policy_evaluates_accepted_candidates_and_selects_validation_best() {
    block_on(async {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let case_set = CaseSet::new(vec![(), ()])
            .with_partition(
                leaven_core::PartitionId::from("TRAIN"),
                vec![leaven_kernel::CaseId::new(0)],
            )
            .with_partition(
                leaven_core::PartitionId::from("VALIDATION"),
                vec![leaven_kernel::CaseId::new(1)],
            );
        let store = InlineEvidenceStore::<ScalarEvidence>::new("inline");
        let mut engine = Engine::<SamplingProblem>::builder()
            .evaluator(ValidationSelectionEvaluator {
                seen_sets: seen.clone(),
            })
            .build();
        let seed = engine
            .insert_seed(
                PartMapArtifact(BTreeMap::from([("answer".to_owned(), "draft".to_owned())])),
                0,
            )
            .unwrap();
        let mut gepa = Gepa::new(
            PartMapSurface,
            ParetoFrontier::by_case().build(),
            FixedSurfaceEdit::new("improved".to_owned()),
        )
        .reflective_dataset(NoReflectiveExamples)
        .validation_policy(FullValidation)
        .max_iterations(1);

        let run = engine.run(&mut gepa, &case_set, &store).await.unwrap();
        let child = engine.view().candidate_tree().children(seed)[0];

        let seen = seen.lock().expect("seen lock").clone();
        assert_eq!(seen.len(), 4);
        assert_eq!(seen[0], vec![leaven_kernel::CaseId::new(0)]);
        assert_eq!(seen[1], vec![leaven_kernel::CaseId::new(1)]);
        assert_eq!(seen[2], vec![leaven_kernel::CaseId::new(0)]);
        assert_eq!(seen[3], vec![leaven_kernel::CaseId::new(1)]);
        assert_eq!(gepa.population().best(), Some(child));
        assert_eq!(run.best, Some(seed));
    });
}

#[test]
fn gepa_checkpoint_restore_rejects_missing_validation_best_candidate() {
    block_on(async {
        let case_set = CaseSet::new(vec![(), ()])
            .with_partition(
                leaven_core::PartitionId::from("TRAIN"),
                vec![leaven_kernel::CaseId::new(0)],
            )
            .with_partition(
                leaven_core::PartitionId::from("VALIDATION"),
                vec![leaven_kernel::CaseId::new(1)],
            );
        let store = InlineEvidenceStore::<ScalarEvidence>::new("inline");
        let mut engine = Engine::<SamplingProblem>::builder()
            .evaluator(ValidationSelectionEvaluator {
                seen_sets: Arc::new(Mutex::new(Vec::new())),
            })
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
            FixedSurfaceEdit::new("improved".to_owned()),
        )
        .reflective_dataset(NoReflectiveExamples)
        .validation_policy(FullValidation)
        .max_iterations(1);
        engine.run(&mut gepa, &case_set, &store).await.unwrap();

        let state = gepa
            .checkpoint_state(CheckpointContext::new(engine.view()))
            .unwrap();
        let mut missing_validation_best = serde_json::to_value(&state).unwrap();
        missing_validation_best["validation_best"]["candidate"] =
            serde_json::to_value(leaven_kernel::CandidateId::new()).unwrap();
        let missing_state = serde_json::from_value(missing_validation_best).unwrap();
        let mut restored = Gepa::new(
            PartMapSurface,
            ParetoFrontier::by_case().build(),
            FixedSurfaceEdit::new("unused".to_owned()),
        )
        .reflective_dataset(NoReflectiveExamples)
        .validation_policy(FullValidation);

        let error = restored
            .restore_state(missing_state, RestoreContext::new(engine.view()))
            .unwrap_err();

        assert!(error.to_string().contains("validation best candidate"));
    });
}

#[test]
fn gepa_checkpoint_state_includes_validation_policy_state() {
    let mut graph = RunGraph::<SmokeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let ctx = RunContext::new(&mut graph, &mut budget);
    let gepa = smoke_gepa(FixedSurfaceEdit::new("unused".to_owned()));

    let state = gepa
        .checkpoint_state(CheckpointContext::new(ctx.graph()))
        .unwrap();
    let serialized = serde_json::to_value(state).unwrap();

    assert!(serialized.get("validation_policy").is_some());
}

#[test]
fn gepa_default_max_iterations_is_not_one_iteration_smoke_config() {
    let mut graph = RunGraph::<SmokeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let ctx = RunContext::new(&mut graph, &mut budget);
    let gepa = smoke_gepa(FixedSurfaceEdit::new("unused".to_owned()));

    let state = gepa
        .checkpoint_state(CheckpointContext::new(ctx.graph()))
        .unwrap();
    let serialized = serde_json::to_value(state).unwrap();

    assert_ne!(serialized["max_iterations"].as_u64(), Some(1));
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
fn gepa_default_validation_policy_is_full_validation() {
    let gepa = Gepa::new(
        PartMapSurface,
        ParetoFrontier::by_case().build(),
        FixedSurfaceEdit::new("improved".to_owned()),
    )
    .reflective_dataset(NoReflectiveExamples);

    assert!(std::any::type_name_of_val(&gepa).contains("FullValidation"));
}

#[test]
fn gepa_run_reports_missing_seed_before_evaluation() {
    block_on(async {
        let case_set = train_case_set();
        let store = InlineEvidenceStore::<SmokeEvidence>::new("inline");
        let mut engine = Engine::<SmokeProblem>::builder()
            .evaluator(VisibilityEvaluator)
            .build();
        let mut gepa = smoke_gepa(FixedSurfaceEdit::new("unused".to_owned()));

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
        let mut gepa = smoke_gepa(FixedSurfaceEdit::new("unused".to_owned()));

        let error = engine.run(&mut gepa, &case_set, &store).await.unwrap_err();

        assert!(error.to_string().contains("comparable case scores"));
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
        let mut gepa = smoke_gepa(FixedSurfaceEdit::new("unused".to_owned())).max_iterations(0);

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

struct SamplingProblem;

impl OptimizationProblem for SamplingProblem {
    type Artifact = PartMapArtifact;
    type Case = ();
    type Evidence = ScalarEvidence;
    type ProposalAnnotations = ();
}

/// Problem with a `Display` case, so the default `GepaReflectiveDataset`
/// builder can read each evaluated case input.
struct DisplayScalarProblem;

impl OptimizationProblem for DisplayScalarProblem {
    type Artifact = PartMapArtifact;
    type Case = &'static str;
    type Evidence = ScalarEvidence;
    type ProposalAnnotations = ();
}

/// Problem with a mixed input/target/metadata case envelope. Its `Display`
/// implementation intentionally leaks hidden fields; the GEPA default builder
/// must use `ReflectiveCaseInput` instead.
struct HiddenTargetProblem;

impl OptimizationProblem for HiddenTargetProblem {
    type Artifact = PartMapArtifact;
    type Case = HiddenTargetCase;
    type Evidence = CaseAssessmentEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug)]
struct HiddenTargetCase {
    safe_input: &'static str,
    hidden_target: &'static str,
    hidden_metadata: &'static str,
}

impl std::fmt::Display for HiddenTargetCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "display leak target={} metadata={}",
            self.hidden_target, self.hidden_metadata
        )
    }
}

impl ReflectiveCaseInput for HiddenTargetCase {
    fn reflective_input(&self) -> String {
        self.safe_input.to_owned()
    }
}

fn independent_train_request(candidate: leaven_kernel::CandidateId) -> EvaluationRequest {
    EvaluationRequest::Independent {
        candidates: vec![candidate],
        set: EvaluationSet::Partition(leaven_core::PartitionId::from("TRAIN")),
        granularity: AssessmentGranularity::PerCase,
        purpose: EvaluationPurpose::SeedBaseline,
    }
}

struct HiddenTargetEvaluator;

impl Evaluator<HiddenTargetProblem> for HiddenTargetEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([12; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, HiddenTargetProblem>,
    ) -> Result<Metered<Vec<Assessment<HiddenTargetProblem>>>, EvaluationError> {
        let set = leaven_kernel::EvaluationSetId::from_uuid(request.set.id.as_uuid());
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        let mut assessments = Vec::new();
        for candidate in candidates {
            for case in request.set.case_ids.iter().copied() {
                assessments.push(Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Case { set, case },
                    evidence: CaseAssessmentEvidence::new(
                        ScalarEvidence::new(0.25).unwrap(),
                        OutputRecord::inline("candidate output"),
                        "visible scorer feedback",
                    ),
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                });
            }
        }
        Ok(Metered::new(
            assessments,
            Cost::metric_calls(request.set.case_ids.len() as u64),
        ))
    }
}

struct ScalarCaseEvaluator;

impl Evaluator<DisplayScalarProblem> for ScalarCaseEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([11; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, DisplayScalarProblem>,
    ) -> Result<Metered<Vec<Assessment<DisplayScalarProblem>>>, EvaluationError> {
        let set = leaven_kernel::EvaluationSetId::from_uuid(request.set.id.as_uuid());
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        let mut assessments = Vec::new();
        for candidate in candidates {
            for case in request.set.case_ids.iter().copied() {
                assessments.push(Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Case { set, case },
                    evidence: ScalarEvidence::new(0.5).unwrap(),
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                });
            }
        }
        Ok(Metered::new(
            assessments,
            Cost::metric_calls(request.set.case_ids.len() as u64),
        ))
    }
}

#[derive(Clone, Debug)]
struct SmokeEvidence;

impl Evidence for SmokeEvidence {}

impl leaven_gepa::GepaCaseEvidence for SmokeEvidence {
    fn scalar_score(&self) -> Option<ScalarEvidence> {
        None
    }
}

/// Test reflective-dataset builder: `SmokeEvidence` is a minimal evidence type
/// with no per-case projection, so smoke tests project no examples.
#[derive(Clone, Copy, Debug, Default)]
struct NoReflectiveExamples;

impl<P, S> ReflectiveDatasetBuilder<P, S> for NoReflectiveExamples
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    async fn build(
        &self,
        _ctx: &mut RunContext<'_, P>,
        _parent: leaven_kernel::CandidateId,
        _parent_assessments: &[leaven_kernel::AssessmentId],
        _part: &S::PartId,
    ) -> Result<Vec<ReflectiveExample>, leaven_gepa::ReflectionError> {
        Ok(Vec::new())
    }
}

/// `SmokeProblem` GEPA value: default strategies plus the no-example dataset
/// builder, because `SmokeEvidence` has no GEPA-parity projection.
type SmokeGepa = Gepa<
    PartMapSurface,
    ParetoFrontier,
    FixedSurfaceEdit<String>,
    ParetoFrequencyWeighted,
    leaven_gepa::RoundRobinPart,
    StrictImprovement,
    EpochShuffled,
    MinibatchThenValidation,
    NoReflectiveExamples,
>;

fn smoke_gepa(reflector: FixedSurfaceEdit<String>) -> SmokeGepa {
    Gepa::new(PartMapSurface, ParetoFrontier::by_case().build(), reflector)
        .validation_policy(MinibatchThenValidation)
        .reflective_dataset(NoReflectiveExamples)
}

fn smoke_gepa_with_population(
    population: ParetoFrontier,
    reflector: FixedSurfaceEdit<String>,
) -> SmokeGepa {
    Gepa::new(PartMapSurface, population, reflector)
        .validation_policy(MinibatchThenValidation)
        .reflective_dataset(NoReflectiveExamples)
}

struct RecordingCaseSetEvaluator {
    seen_sets: Arc<Mutex<Vec<Vec<leaven_kernel::CaseId>>>>,
}

impl Evaluator<SamplingProblem> for RecordingCaseSetEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([8; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'_, SamplingProblem>,
    ) -> Result<Metered<Vec<Assessment<SamplingProblem>>>, EvaluationError> {
        self.seen_sets
            .lock()
            .expect("seen sets lock")
            .push(request.set.case_ids.clone());
        let set = leaven_kernel::EvaluationSetId::from_uuid(request.set.id.as_uuid());
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        let mut assessments = Vec::new();
        for candidate in candidates {
            let artifact = ctx.graph().artifact(candidate).expect("candidate artifact");
            let score = if artifact.0.get("answer").map(String::as_str) == Some("improved") {
                1.0
            } else {
                0.0
            };
            for case in request.set.case_ids.iter().copied() {
                assessments.push(Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Case { set, case },
                    evidence: ScalarEvidence::new(score).unwrap(),
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                });
            }
        }
        Ok(Metered::new(
            assessments,
            Cost::metric_calls(request.set.case_ids.len() as u64),
        ))
    }
}

struct ValidationSelectionEvaluator {
    seen_sets: Arc<Mutex<Vec<Vec<leaven_kernel::CaseId>>>>,
}

impl Evaluator<SamplingProblem> for ValidationSelectionEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([9; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'_, SamplingProblem>,
    ) -> Result<Metered<Vec<Assessment<SamplingProblem>>>, EvaluationError> {
        self.seen_sets
            .lock()
            .expect("seen sets lock")
            .push(request.set.case_ids.clone());
        let set = leaven_kernel::EvaluationSetId::from_uuid(request.set.id.as_uuid());
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        let mut assessments = Vec::new();
        for candidate in candidates {
            let artifact = ctx.graph().artifact(candidate).expect("candidate artifact");
            let improved = artifact.0.get("answer").map(String::as_str) == Some("improved");
            for case in request.set.case_ids.iter().copied() {
                let score = if case == leaven_kernel::CaseId::new(1) {
                    if improved { 0.0 } else { 1.0 }
                } else if improved {
                    1.0
                } else {
                    0.0
                };
                assessments.push(Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Case { set, case },
                    evidence: ScalarEvidence::new(score).unwrap(),
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                });
            }
        }
        Ok(Metered::new(
            assessments,
            Cost::metric_calls(request.set.case_ids.len() as u64),
        ))
    }
}

struct PrefixImprovementEvaluator;

impl Evaluator<SamplingProblem> for PrefixImprovementEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([10; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'_, SamplingProblem>,
    ) -> Result<Metered<Vec<Assessment<SamplingProblem>>>, EvaluationError> {
        let set = leaven_kernel::EvaluationSetId::from_uuid(request.set.id.as_uuid());
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        let mut assessments = Vec::new();
        for candidate in candidates {
            let artifact = ctx.graph().artifact(candidate).expect("candidate artifact");
            let score = if artifact
                .0
                .get("answer")
                .is_some_and(|value| value.starts_with("improved"))
            {
                1.0
            } else {
                0.0
            };
            for case in request.set.case_ids.iter().copied() {
                assessments.push(Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Case { set, case },
                    evidence: ScalarEvidence::new(score).unwrap(),
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                });
            }
        }
        Ok(Metered::new(
            assessments,
            Cost::metric_calls(request.set.case_ids.len() as u64),
        ))
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
        let set = leaven_kernel::EvaluationSetId::from_uuid(request.set.id.as_uuid());
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        let mut assessments = Vec::new();
        for candidate in candidates {
            match request.granularity {
                AssessmentGranularity::PerCase | AssessmentGranularity::Both => {
                    for case in request.set.case_ids.iter().copied() {
                        assessments.push(Assessment::Independent {
                            candidate,
                            target: AssessmentTarget::Case { set, case },
                            evidence: SmokeEvidence,
                            cost: Cost::metric_calls(1),
                            metadata: MetadataBag::new(),
                        });
                    }
                }
                AssessmentGranularity::Aggregate => {
                    assessments.push(Assessment::Independent {
                        candidate,
                        target: AssessmentTarget::EvaluationSet(set),
                        evidence: SmokeEvidence,
                        cost: Cost::metric_calls(1),
                        metadata: MetadataBag::new(),
                    });
                }
            }
        }
        let cost = Cost::metric_calls(assessments.len() as u64);
        Ok(Metered::new(assessments, cost))
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
struct SequentialSurfaceEdits {
    edits: std::collections::VecDeque<String>,
}

impl SequentialSurfaceEdits {
    fn new<const N: usize>(edits: [&str; N]) -> Self {
        Self {
            edits: edits.into_iter().map(str::to_owned).collect(),
        }
    }
}

impl GepaReflector<SamplingProblem, PartMapSurface> for SequentialSurfaceEdits {
    async fn reflect_candidate(
        &mut self,
        ctx: &mut RunContext<'_, SamplingProblem>,
        surface: &PartMapSurface,
        request: ReflectRequest<String>,
    ) -> Result<Option<leaven_kernel::CandidateId>, leaven_engine::OptimizerError> {
        let edit = self.edits.pop_front().expect("enough scripted edits");
        FixedSurfaceEdit::new(edit)
            .reflect_candidate(ctx, surface, request)
            .await
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
