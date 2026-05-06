mod support;

use leaven_core::{CausalInputs, InfoRef, Proposal, ProposalEffect, ProposalProvenance};
use leaven_engine::{ApplyOutcome, CandidateOrigin, RunContext, RunEvent};
use leaven_kernel::{ErrorKind, MetadataBag};
use proptest::prelude::*;

use support::{TestProblem, TextArtifact, TextChange, graph_and_budget, record_one};

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

    let proposal = Proposal::mutate(seed, TextChange::Append("b")).build();
    let batch = record_one(&mut ctx, proposal);
    let report = ctx.apply_batch(batch).unwrap();
    let child = report.successful_candidates().next().unwrap();

    let view = ctx.graph();
    assert_eq!(view.parents(child), [seed]);
    assert_eq!(view.children(seed), [child]);
}

#[test]
fn invalid_change_provenance_records_failed_apply() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(TextArtifact("a".to_owned()), 0).unwrap();
    let proposal = Proposal {
        effect: ProposalEffect::Change {
            target: seed,
            change: TextChange::Append("b"),
        },
        provenance: ProposalProvenance::new(CausalInputs::None),
        annotations: (),
        metadata: MetadataBag::new(),
    };

    let batch = record_one(&mut ctx, proposal);
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

    let proposal = Proposal::mutate(target, TextChange::Append("!"))
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

    let proposal = Proposal::merge(left, right, TextChange::Append("+merged")).build();
    let batch = record_one(&mut ctx, proposal);
    let report = ctx.apply_batch(batch).unwrap();
    let child = report.successful_candidates().next().unwrap();

    let view = ctx.graph();
    assert_eq!(view.artifact(child).unwrap().0, "left+merged");
    assert_eq!(view.parents(child), [left, right]);
    assert_eq!(view.children(left), [child]);
    assert_eq!(view.children(right), [child]);
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
                    Proposal::mutate(seed, TextChange::Append("+ok")).build(),
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
                Proposal::mutate(*known.last().unwrap(), TextChange::Append("x")).build()
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
