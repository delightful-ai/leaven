use leaven_core::{
    Assessment, AssessmentGranularity, AssessmentTarget, CausalInputs, EvaluationPurpose,
    EvaluationRequest, EvaluationSet, InfoRef, Proposal, ProposalEffect, ProposalProvenance,
    ResolvedEvaluationRequest, Window,
};
use leaven_engine::{
    ApplyOutcome, CachePolicy, CandidateOrigin, EvaluationContext, EvaluationError, Evaluator,
    RunContext, RunEvent, RunGraph, RunGraphRestoreError,
};
use leaven_kernel::{
    ApplyAttemptId, Cost, ErrorKind, EvaluatorId, Fingerprint, MetadataBag, MetadataKey,
    MetadataValue, Metered, StageId,
};
use leaven_store_inline::InlineEvidenceStore;
use proptest::prelude::*;

use super::support::{
    TestEvidence, TestProblem, TextArtifact, TextChange, graph_and_budget, record_one,
};

#[test]
fn create_proposal_creates_candidate_without_causal_parent() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);

    let proposal = Proposal::create(TextArtifact("fresh".to_owned())).build();
    let batch = record_one(&mut ctx, proposal);
    let report = ctx.apply_batch(batch).unwrap();
    let candidate = report.successful_candidates().next().unwrap();

    let view = ctx.graph();
    let candidate_view = view.candidate(candidate).unwrap();
    let batch_view = view.proposal_batch(batch).unwrap();
    let proposal_id = batch_view.proposal_ids()[0];
    let proposal_view = view.proposal_that_created(candidate).unwrap();
    assert_eq!(candidate_view.id(), candidate);
    assert!(matches!(
        candidate_view.origin(),
        CandidateOrigin::Proposal { .. }
    ));
    assert!(candidate_view.created_at() <= leaven_kernel::now());
    assert_eq!(batch_view.id(), batch);
    assert_eq!(
        batch_view.semantics(),
        leaven_core::ProposalBatchSemantics::Alternatives
    );
    assert_eq!(proposal_view.id(), proposal_id);
    assert_eq!(view.parents(candidate), []);
}

#[test]
fn change_proposal_creates_causal_edge() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(TextArtifact("a".to_owned()), 0).unwrap();

    let proposal = Proposal::mutate(seed, TextChange::Append("b".to_owned())).build();
    let batch = record_one(&mut ctx, proposal);
    let report = ctx.apply_batch(batch).unwrap();
    let child = report.successful_candidates().next().unwrap();

    let view = ctx.graph();
    assert_eq!(view.parents(child), [seed]);
    assert_eq!(view.children(seed), [child]);
    let lineage = view.lineage(child);
    assert_eq!(lineage.root(), child);
    assert_eq!(lineage.parents(), [seed]);
    assert_eq!(lineage.ancestors(), [seed]);
    assert!(lineage.contains(seed));
    let tree = view.candidate_tree();
    assert!(tree.contains(seed));
    assert_eq!(tree.roots(), [seed]);
    assert_eq!(tree.parents(child), [seed]);
    assert_eq!(tree.children(seed), [child]);
}

#[test]
fn graph_snapshot_round_trips_and_rebuilds_lineage_indices() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(TextArtifact("a".to_owned()), 0).unwrap();
    let proposal = Proposal::mutate(seed, TextChange::Append("b".to_owned())).build();
    let batch = record_one(&mut ctx, proposal);
    let report = ctx.apply_batch(batch).unwrap();
    let child = report.successful_candidates().next().unwrap();
    drop(ctx);

    let encoded = serde_json::to_vec(&graph.snapshot()).unwrap();
    let decoded = serde_json::from_slice(&encoded).unwrap();
    let mut restored = RunGraph::<TestProblem>::from_snapshot(decoded).unwrap();
    let mut restored_budget = leaven_engine::BudgetLedger::new(leaven_kernel::Budget::unlimited());
    let restored_ctx = RunContext::<TestProblem>::new(&mut restored, &mut restored_budget);
    let view = restored_ctx.graph();

    assert_eq!(view.parents(child), [seed]);
    assert_eq!(view.children(seed), [child]);
    assert_eq!(view.proposal_that_created(child).unwrap().batch_id(), batch);
}

#[test]
fn graph_snapshot_restore_rejects_dangling_references() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(TextArtifact("a".to_owned()), 0).unwrap();
    let proposal = Proposal::mutate(seed, TextChange::Append("b".to_owned())).build();
    let batch = record_one(&mut ctx, proposal);
    let report = ctx.apply_batch(batch).unwrap();
    report.successful_candidates().next().unwrap();
    drop(ctx);

    let snapshot = graph.snapshot();
    let proposal_id = snapshot.proposals[0].id;
    let attempt_id = snapshot.apply_attempts[0].id;

    let mut missing_proposal = snapshot.clone();
    missing_proposal.proposals.clear();
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(missing_proposal),
        Err(RunGraphRestoreError::MissingProposalInBatch { proposal, .. })
            if proposal == proposal_id
    ));

    let mut missing_attempt = snapshot;
    missing_attempt.apply_attempts.clear();
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(missing_attempt),
        Err(RunGraphRestoreError::MissingApplyAttemptForCandidate { attempt, .. })
            if attempt == attempt_id
    ));
}

#[test]
fn graph_snapshot_restore_rejects_duplicate_record_ids() {
    let snapshot = snapshot_with_proposal_and_assessment();

    let mut duplicate_candidate = snapshot.clone();
    duplicate_candidate
        .candidates
        .push(duplicate_candidate.candidates[0].clone());
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(duplicate_candidate),
        Err(RunGraphRestoreError::DuplicateCandidate(_))
    ));

    let mut duplicate_batch = snapshot.clone();
    duplicate_batch
        .proposal_batches
        .push(duplicate_batch.proposal_batches[0].clone());
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(duplicate_batch),
        Err(RunGraphRestoreError::DuplicateProposalBatch(_))
    ));

    let mut duplicate_proposal = snapshot.clone();
    duplicate_proposal
        .proposals
        .push(duplicate_proposal.proposals[0].clone());
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(duplicate_proposal),
        Err(RunGraphRestoreError::DuplicateProposal(_))
    ));

    let mut duplicate_attempt = snapshot.clone();
    duplicate_attempt
        .apply_attempts
        .push(duplicate_attempt.apply_attempts[0].clone());
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(duplicate_attempt),
        Err(RunGraphRestoreError::DuplicateApplyAttempt(_))
    ));

    let mut duplicate_request = snapshot.clone();
    duplicate_request
        .evaluation_requests
        .push(duplicate_request.evaluation_requests[0].clone());
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(duplicate_request),
        Err(RunGraphRestoreError::DuplicateEvaluationRequest(_))
    ));

    let mut duplicate_assessment = snapshot;
    duplicate_assessment
        .assessments
        .push(duplicate_assessment.assessments[0].clone());
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(duplicate_assessment),
        Err(RunGraphRestoreError::DuplicateAssessment(_))
    ));
}

#[test]
fn graph_snapshot_restore_rejects_proposal_batch_inconsistencies() {
    let snapshot = snapshot_with_proposal_and_assessment();
    let proposal_id = snapshot.proposals[0].id;
    let batch_id = snapshot.proposal_batches[0].id;

    let mut missing_batch = snapshot.clone();
    missing_batch.proposal_batches.clear();
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(missing_batch),
        Err(RunGraphRestoreError::MissingBatchForProposal { proposal, batch })
            if proposal == proposal_id && batch == batch_id
    ));

    let mut proposal_not_listed = snapshot.clone();
    proposal_not_listed.proposal_batches[0].proposal_ids.clear();
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(proposal_not_listed),
        Err(RunGraphRestoreError::ProposalNotListedInBatch { proposal, batch })
            if proposal == proposal_id && batch == batch_id
    ));

    let mut invalid_proposal = snapshot;
    invalid_proposal.proposals[0].provenance = ProposalProvenance::new(CausalInputs::None);
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(invalid_proposal),
        Err(RunGraphRestoreError::InvalidRestoredProposal { proposal, .. })
            if proposal == proposal_id
    ));
}

#[test]
fn graph_snapshot_restore_rejects_apply_and_candidate_origin_inconsistencies() {
    let snapshot = snapshot_with_proposal_and_assessment();
    let proposal_id = snapshot.proposals[0].id;
    let attempt_id = snapshot.apply_attempts[0].id;
    let child = snapshot
        .candidates
        .iter()
        .find(|candidate| matches!(candidate.origin, CandidateOrigin::Proposal { .. }))
        .unwrap()
        .id;

    let mut missing_proposal_for_attempt = snapshot.clone();
    missing_proposal_for_attempt.proposals.clear();
    missing_proposal_for_attempt.proposal_batches[0]
        .proposal_ids
        .clear();
    missing_proposal_for_attempt
        .candidates
        .retain(|candidate| candidate.id != child);
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(missing_proposal_for_attempt),
        Err(RunGraphRestoreError::MissingProposalForApplyAttempt { attempt, proposal })
            if attempt == attempt_id && proposal == proposal_id
    ));

    let mut missing_candidate_for_attempt = snapshot.clone();
    missing_candidate_for_attempt
        .candidates
        .retain(|candidate| candidate.id != child);
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(missing_candidate_for_attempt),
        Err(RunGraphRestoreError::MissingCandidateForSuccessfulApplyAttempt {
            attempt,
            candidate,
        }) if attempt == attempt_id && candidate == child
    ));

    let mut apply_candidate_mismatch = snapshot.clone();
    apply_candidate_mismatch.candidates[1].origin = CandidateOrigin::Seed { seed_index: 99 };
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(apply_candidate_mismatch),
        Err(RunGraphRestoreError::ApplyAttemptCandidateMismatch { attempt, candidate })
            if attempt == attempt_id && candidate == child
    ));

    let mut missing_proposal_for_candidate = snapshot.clone();
    missing_proposal_for_candidate.proposals.clear();
    missing_proposal_for_candidate.proposal_batches[0]
        .proposal_ids
        .clear();
    missing_proposal_for_candidate.apply_attempts.clear();
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(missing_proposal_for_candidate),
        Err(RunGraphRestoreError::MissingProposalForCandidate {
            candidate,
            proposal,
        }) if candidate == child && proposal == proposal_id
    ));

    let mut missing_attempt_for_candidate = snapshot.clone();
    missing_attempt_for_candidate.apply_attempts.clear();
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(missing_attempt_for_candidate),
        Err(RunGraphRestoreError::MissingApplyAttemptForCandidate { candidate, attempt })
            if candidate == child && attempt == attempt_id
    ));

    let mut candidate_attempt_mismatch = snapshot;
    candidate_attempt_mismatch
        .apply_attempts
        .push(candidate_attempt_mismatch.apply_attempts[0].clone());
    let replacement_attempt = ApplyAttemptId::new();
    candidate_attempt_mismatch.apply_attempts[1].id = replacement_attempt;
    let child_record = candidate_attempt_mismatch
        .candidates
        .iter_mut()
        .find(|candidate| candidate.id == child)
        .unwrap();
    child_record.origin = CandidateOrigin::Proposal {
        proposal_id,
        apply_attempt_id: replacement_attempt,
    };
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(candidate_attempt_mismatch),
        Err(RunGraphRestoreError::ApplyAttemptCandidateMismatch { .. })
    ));
}

#[test]
fn graph_snapshot_restore_rejects_assessment_inconsistencies() {
    let snapshot = snapshot_with_seed_assessment();
    let assessment_id = snapshot.assessments[0].id;
    let request_id = snapshot.evaluation_requests[0].id;
    let candidate_id = snapshot.candidates[0].id;

    let mut missing_request = snapshot.clone();
    missing_request.evaluation_requests.clear();
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(missing_request),
        Err(RunGraphRestoreError::MissingEvaluationRequestForAssessment {
            assessment,
            request,
        }) if assessment == assessment_id && request == request_id
    ));

    let mut missing_candidate = snapshot;
    missing_candidate.candidates.clear();
    assert!(matches!(
        RunGraph::<TestProblem>::from_snapshot(missing_candidate),
        Err(RunGraphRestoreError::MissingCandidateForAssessment {
            assessment,
            candidate,
        }) if assessment == assessment_id && candidate == candidate_id
    ));
}

#[test]
fn graph_views_expose_record_details_without_storage_maps() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let stage = StageId::custom("detail-proposer");
    let mut batch_metadata = MetadataBag::new();
    batch_metadata.insert("batch", MetadataValue::Bool(true));
    let mut proposal_metadata = MetadataBag::new();
    proposal_metadata.insert("proposal", MetadataValue::String("detail".to_owned()));
    let proposal = Proposal::create(TextArtifact("detailed".to_owned()))
        .metadata(proposal_metadata)
        .build();
    let report = ctx
        .record_proposal_batch(
            stage.clone(),
            leaven_core::ProposalBatch {
                proposals: vec![proposal],
                semantics: leaven_core::ProposalBatchSemantics::Alternatives,
                metadata: batch_metadata,
            },
            Cost::zero(),
        )
        .unwrap();
    let candidate = ctx
        .apply_batch(report.batch_id)
        .unwrap()
        .successful_candidates()
        .next()
        .unwrap();

    let view = ctx.graph();
    let batch = view.proposal_batch(report.batch_id).unwrap();
    let proposal = view.proposal_that_created(candidate).unwrap();
    let proposal_by_id = view.proposal(proposal.id()).unwrap();

    assert_eq!(view.candidate_count(), 1);
    assert_eq!(
        view.proposal_batches()
            .map(|batch| batch.id())
            .collect::<Vec<_>>(),
        [report.batch_id]
    );
    assert_eq!(batch.stage(), &stage);
    assert_eq!(proposal_by_id.id(), proposal.id());
    assert_eq!(proposal_by_id.batch_id(), report.batch_id);
    assert!(matches!(
        batch.metadata().get(&MetadataKey::from("batch")),
        Some(MetadataValue::Bool(true))
    ));
    assert!(batch.created_at() <= leaven_kernel::now());
    assert_eq!(batch.iteration(), None);
    assert_eq!(proposal.batch_id(), report.batch_id);
    assert!(matches!(
        proposal.effect(),
        ProposalEffect::Create {
            artifact: TextArtifact(text)
        } if text == "detailed"
    ));
    assert_eq!(proposal.provenance().causal(), &CausalInputs::None);
    assert_eq!(proposal.annotations(), &());
    assert!(matches!(
        proposal.metadata().get(&MetadataKey::from("proposal")),
        Some(MetadataValue::String(value)) if value == "detail"
    ));
    assert!(proposal.created_at() <= leaven_kernel::now());
}

#[test]
fn invalid_change_provenance_records_failed_apply() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(TextArtifact("a".to_owned()), 0).unwrap();
    let proposal = Proposal {
        effect: ProposalEffect::Change {
            target: seed,
            change: TextChange::Append("b".to_owned()),
        },
        provenance: ProposalProvenance::new(CausalInputs::None),
        annotations: (),
        metadata: MetadataBag::new(),
    };

    let batch = record_one(&mut ctx, proposal);
    let proposal_id = ctx.graph().proposal_batch(batch).unwrap().proposal_ids()[0];
    let report = ctx.apply_batch(batch).unwrap();

    assert!(matches!(
        report.outcomes[0].outcome,
        ApplyOutcome::Failure { .. }
    ));
    let view = ctx.graph();
    let events = view.events().collect::<Vec<_>>();
    assert!(matches!(
        events.as_slice(),
        [
            RunEvent::BudgetCharged { .. },
            RunEvent::ProposalBatchProduced { .. },
            RunEvent::ProposalRecorded { .. },
            RunEvent::ApplyFailed { .. },
            RunEvent::Error { .. },
        ]
    ));
    let failures = view.recent_failures(Window { limit: 1 });
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].id(), report.outcomes[0].attempt_id);
    assert_eq!(failures[0].proposal_id(), proposal_id);
    assert_eq!(failures[0].error().kind, ErrorKind::GraphInvariant);
    assert!(failures[0].created_at() <= leaven_kernel::now());

    let successful_batch = record_one(
        &mut ctx,
        Proposal::mutate(seed, TextChange::Append("c".to_owned())).build(),
    );
    let successful_report = ctx.apply_batch(successful_batch).unwrap();
    assert_eq!(successful_report.successful_candidates().count(), 1);
    assert_eq!(ctx.graph().recent_failures(Window { limit: 10 }).len(), 1);
}

#[test]
fn create_proposals_reject_causal_single_parent_provenance() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap();
    let proposal = Proposal {
        effect: ProposalEffect::Create {
            artifact: TextArtifact("fresh".to_owned()),
        },
        provenance: ProposalProvenance::new(CausalInputs::Single(seed)),
        annotations: (),
        metadata: MetadataBag::new(),
    };

    let batch = record_one(&mut ctx, proposal);
    let report = ctx.apply_batch(batch).unwrap();

    assert!(matches!(
        report.outcomes[0].outcome,
        ApplyOutcome::Failure { .. }
    ));
    assert!(
        ctx.graph()
            .events()
            .any(|event| matches!(event, RunEvent::ApplyFailed { .. }))
    );
}

#[test]
fn aggregate_create_records_nary_lineage_and_rejects_unknown_parents() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let left = ctx.insert_seed(TextArtifact("left".to_owned()), 0).unwrap();
    let right = ctx
        .insert_seed(TextArtifact("right".to_owned()), 1)
        .unwrap();

    let ok_batch = record_one(
        &mut ctx,
        Proposal::aggregate(vec![left, right], TextArtifact("joined".to_owned())).build(),
    );
    let ok = ctx.apply_batch(ok_batch).unwrap();
    let child = ok.successful_candidates().next().unwrap();

    assert_eq!(ctx.graph().parents(child), [left, right]);
    assert_eq!(ctx.graph().lineage(child).ancestors(), [left, right]);

    let bad_batch = record_one(
        &mut ctx,
        Proposal::aggregate(
            vec![leaven_kernel::CandidateId::new()],
            TextArtifact("bad".to_owned()),
        )
        .build(),
    );
    let bad = ctx.apply_batch(bad_batch).unwrap();

    assert!(matches!(
        bad.outcomes[0].outcome,
        ApplyOutcome::Failure { .. }
    ));
}

#[test]
fn informed_by_does_not_affect_lineage() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let source = ctx
        .insert_seed(TextArtifact("source".to_owned()), 0)
        .unwrap();
    let target = ctx
        .insert_seed(TextArtifact("target".to_owned()), 1)
        .unwrap();

    let proposal = Proposal::mutate(target, TextChange::Append("!".to_owned()))
        .informed_by([InfoRef::Candidate(source)])
        .build();
    let batch = record_one(&mut ctx, proposal);
    let report = ctx.apply_batch(batch).unwrap();
    let child = report.successful_candidates().next().unwrap();

    let view = ctx.graph();
    assert_eq!(view.parents(child), [target]);
    assert_eq!(view.informed_by(child), [InfoRef::Candidate(source)]);
    assert_eq!(view.informed(source), [child]);
}

#[test]
fn merge_proposal_records_pair_lineage_but_applies_to_one_target() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = leaven_engine::RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let left = ctx.insert_seed(TextArtifact("left".to_owned()), 0).unwrap();
    let right = ctx
        .insert_seed(TextArtifact("right".to_owned()), 1)
        .unwrap();

    let proposal = Proposal::merge(left, right, TextChange::Append("+merged".to_owned())).build();
    let batch = record_one(&mut ctx, proposal);
    let report = ctx.apply_batch(batch).unwrap();
    let child = report.successful_candidates().next().unwrap();

    let view = ctx.graph();
    assert_eq!(view.artifact(child).unwrap().0, "left+merged");
    assert_eq!(view.parents(child), [left, right]);
    assert_eq!(view.children(left), [child]);
    assert_eq!(view.children(right), [child]);
    assert_eq!(view.lineage(child).parents(), [left, right]);
}

#[test]
fn lineage_ancestors_deduplicate_diamonds_without_dropping_parents() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = leaven_engine::RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap();

    let left_batch = record_one(
        &mut ctx,
        Proposal::mutate(seed, TextChange::Append("+left".to_owned())).build(),
    );
    let right_batch = record_one(
        &mut ctx,
        Proposal::mutate(seed, TextChange::Append("+right".to_owned())).build(),
    );
    let left = ctx
        .apply_batch(left_batch)
        .unwrap()
        .successful_candidates()
        .next()
        .unwrap();
    let right = ctx
        .apply_batch(right_batch)
        .unwrap()
        .successful_candidates()
        .next()
        .unwrap();
    let merge_batch = record_one(
        &mut ctx,
        Proposal::merge(left, right, TextChange::Append("+merged".to_owned())).build(),
    );
    let merged = ctx
        .apply_batch(merge_batch)
        .unwrap()
        .successful_candidates()
        .next()
        .unwrap();

    let lineage = ctx.graph().lineage(merged);
    assert_eq!(lineage.parents(), [left, right]);
    assert_eq!(lineage.ancestors(), [left, right, seed]);
}

#[test]
fn siblings_are_candidates_that_share_causal_parents() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = leaven_engine::RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap();

    let first_batch = record_one(
        &mut ctx,
        Proposal::mutate(seed, TextChange::Append("+a".to_owned())).build(),
    );
    let second_batch = record_one(
        &mut ctx,
        Proposal::mutate(seed, TextChange::Append("+b".to_owned())).build(),
    );
    let first = ctx
        .apply_batch(first_batch)
        .unwrap()
        .successful_candidates()
        .next()
        .unwrap();
    let second = ctx
        .apply_batch(second_batch)
        .unwrap()
        .successful_candidates()
        .next()
        .unwrap();

    let view = ctx.graph();
    assert_eq!(view.siblings(first), [second]);
    assert_eq!(view.siblings(second), [first]);
}

#[test]
fn same_content_can_have_multiple_candidates() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = leaven_engine::RunContext::<TestProblem>::new(&mut graph, &mut budget);

    let first_batch = record_one(
        &mut ctx,
        Proposal::create(TextArtifact("same".to_owned())).build(),
    );
    let second_batch = record_one(
        &mut ctx,
        Proposal::create(TextArtifact("same".to_owned())).build(),
    );
    let first = ctx
        .apply_batch(first_batch)
        .unwrap()
        .successful_candidates()
        .next()
        .unwrap();
    let second = ctx
        .apply_batch(second_batch)
        .unwrap()
        .successful_candidates()
        .next()
        .unwrap();

    let view = ctx.graph();
    assert_ne!(first, second);
    assert_eq!(
        view.candidate(first).unwrap().identity(),
        view.candidate(second).unwrap().identity()
    );
    assert_eq!(
        view.candidates_with_identity(view.candidate(first).unwrap().identity()),
        vec![first, second]
    );
}

#[test]
fn applying_same_proposal_twice_is_rejected_without_second_candidate() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = leaven_engine::RunContext::<TestProblem>::new(&mut graph, &mut budget);

    let batch = record_one(
        &mut ctx,
        Proposal::create(TextArtifact("once".to_owned())).build(),
    );
    let proposal_id = ctx.graph().proposal_batch(batch).unwrap().proposal_ids()[0];
    let first = ctx.apply_proposal(proposal_id).unwrap();
    let second = ctx.apply_proposal(proposal_id).unwrap();

    assert!(matches!(first.outcome, ApplyOutcome::Success { .. }));
    let ApplyOutcome::Failure { error } = second.outcome else {
        panic!("second apply should fail");
    };
    assert_eq!(error.kind, ErrorKind::GraphInvariant);
    assert_eq!(ctx.graph().candidate_count(), 1);
}

#[test]
fn apply_batch_reports_partial_failure_without_aborting() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = leaven_engine::RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap();

    let batch = ctx
        .record_proposal_batch(
            leaven_kernel::StageId::custom("test"),
            leaven_core::ProposalBatch {
                proposals: vec![
                    Proposal::mutate(seed, TextChange::Append("+ok".to_owned())).build(),
                    Proposal::mutate(seed, TextChange::Fail).build(),
                ],
                semantics: leaven_core::ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            leaven_kernel::Cost::zero(),
        )
        .unwrap()
        .batch_id;

    let report = ctx.apply_batch(batch).unwrap();

    assert_eq!(report.outcomes.len(), 2);
    assert_eq!(report.successful_candidates().count(), 1);
    assert!(matches!(
        report.outcomes[0].outcome,
        ApplyOutcome::Success { .. }
    ));
    assert!(matches!(
        report.outcomes[1].outcome,
        ApplyOutcome::Failure { .. }
    ));
    assert_eq!(ctx.graph().candidate_count(), 2);
}

proptest! {
    #[test]
    fn graph_records_are_append_only(ops in proptest::collection::vec(any::<bool>(), 0..32)) {
        let (mut graph, mut budget) = graph_and_budget();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let seed = ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap();
        let seed_artifact = ctx.graph().artifact(seed).unwrap().clone();
        let mut known = vec![seed];
        let mut previous = graph_counts(&ctx);

        for create in ops {
            let proposal = if create {
                Proposal::create(TextArtifact("fresh".to_owned())).build()
            } else {
                Proposal::mutate(*known.last().unwrap(), TextChange::Append("x".to_owned()))
                    .build()
            };
            let batch = record_one(&mut ctx, proposal);
            let report = ctx.apply_batch(batch).unwrap();
            known.extend(report.successful_candidates());

            let next = graph_counts(&ctx);
            prop_assert!(next.candidates >= previous.candidates);
            prop_assert!(next.proposal_batches >= previous.proposal_batches);
            prop_assert!(next.proposals >= previous.proposals);
            prop_assert!(next.apply_attempts >= previous.apply_attempts);
            prop_assert!(next.events >= previous.events);
            prop_assert_eq!(ctx.graph().artifact(seed), Some(&seed_artifact));
            previous = next;
        }
    }
}

#[derive(Clone, Copy)]
struct GraphCounts {
    candidates: usize,
    proposal_batches: usize,
    proposals: usize,
    apply_attempts: usize,
    events: usize,
}

fn graph_counts(ctx: &RunContext<'_, TestProblem>) -> GraphCounts {
    let graph = ctx.graph();
    GraphCounts {
        candidates: graph.candidate_count(),
        proposal_batches: graph.proposal_batch_count(),
        proposals: graph.proposal_count(),
        apply_attempts: graph.apply_attempt_count(),
        events: graph.event_count(),
    }
}

fn snapshot_with_proposal_and_assessment() -> leaven_engine::RunGraphSnapshot<TestProblem> {
    futures::executor::block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let store = InlineEvidenceStore::<TestEvidence>::new("graph-restore");
        let case_set = leaven_engine::CaseSet::new(vec!["case"]);
        let child = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            let seed = ctx.insert_seed(TextArtifact("a".to_owned()), 0).unwrap();
            let proposal = Proposal::mutate(seed, TextChange::Append("b".to_owned())).build();
            let batch = record_one(&mut ctx, proposal);
            ctx.apply_batch(batch)
                .unwrap()
                .successful_candidates()
                .next()
                .unwrap()
        };
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_evidence_store(&store);
        ctx.evaluate_with(&GraphRestoreEvaluator, independent_request(child))
            .await
            .unwrap();
        graph.snapshot()
    })
}

fn snapshot_with_seed_assessment() -> leaven_engine::RunGraphSnapshot<TestProblem> {
    futures::executor::block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let store = InlineEvidenceStore::<TestEvidence>::new("graph-restore");
        let case_set = leaven_engine::CaseSet::new(vec!["case"]);
        let seed = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact("a".to_owned()), 0).unwrap()
        };
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_evidence_store(&store);
        ctx.evaluate_with(&GraphRestoreEvaluator, independent_request(seed))
            .await
            .unwrap();
        graph.snapshot()
    })
}

fn independent_request(candidate: leaven_kernel::CandidateId) -> EvaluationRequest {
    EvaluationRequest::Independent {
        candidates: vec![candidate],
        set: EvaluationSet::All,
        granularity: AssessmentGranularity::Aggregate,
        purpose: EvaluationPurpose::Search,
    }
}

struct GraphRestoreEvaluator;

impl Evaluator<TestProblem> for GraphRestoreEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::from("graph-restore")
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
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                })
                .collect(),
            Cost::metric_calls(1),
        ))
    }
}
