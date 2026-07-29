use leaven_core::{
    AssessmentGranularity, EvaluationPurpose, EvaluationRequest, EvaluationSet, PairOrder,
    PartitionId,
};
use leaven_engine::{Actor, CaseSet, EvidenceVisibility, TrustPolicy};
use leaven_kernel::{CandidateId, CaseId, EvaluatorId, ProposerId, RendererId};

#[test]
fn read_scopes_preserve_hidden_partitions_by_actor() {
    let secret = PartitionId::from("secret");
    let policy = TrustPolicy::default()
        .hide_from_proposers([secret.clone()])
        .hide_from_optimizers([secret.clone()])
        .hide_from_callbacks([secret.clone()]);

    assert!(
        policy
            .proposer_read_scope()
            .hidden_partitions
            .contains(&secret)
    );
    assert!(
        policy
            .optimizer_read_scope()
            .hidden_partitions
            .contains(&secret)
    );
    assert!(policy.evaluator_read_scope().hidden_partitions.is_empty());
    assert!(
        policy
            .callback_read_scope()
            .hidden_partitions
            .contains(&secret)
    );
    assert_eq!(
        policy.evaluator_read_scope().visible_evidence,
        EvidenceVisibility::Full
    );
}

#[test]
fn hidden_partition_requests_are_rejected_for_optimizers_and_proposers() {
    let secret = PartitionId::from("secret");
    let policy = TrustPolicy::default()
        .hide_from_proposers([secret.clone()])
        .hide_from_optimizers([secret.clone()])
        .hide_from_callbacks([secret.clone()]);
    let request = independent(EvaluationSet::Partition(secret.clone()), EvaluationPurpose::Search);

    assert_hidden_partition_refusal(
        policy.check_evaluation_request(&Actor::Optimizer, &request),
        &secret,
    );
    assert_hidden_partition_refusal(
        policy.check_evaluation_request(&Actor::Proposer(ProposerId::from("p")), &request),
        &secret,
    );
    assert_hidden_partition_refusal(
        policy.check_evaluation_request(&Actor::Callback, &request),
        &secret,
    );
    assert!(
        policy
            .check_evaluation_request(&Actor::Evaluator(EvaluatorId::PRIMARY), &request)
            .is_ok()
    );
    assert!(
        policy
            .check_evaluation_request(&Actor::Renderer(RendererId::from("r")), &request)
            .is_ok()
    );
}

#[test]
fn nested_sets_that_reference_hidden_partitions_are_rejected() {
    let secret = PartitionId::from("secret");
    let public = PartitionId::from("public");
    let policy = TrustPolicy::default().hide_from_optimizers([secret.clone()]);

    for set in [
        EvaluationSet::All,
        EvaluationSet::Sample {
            of: Box::new(EvaluationSet::Partition(secret.clone())),
            n: 1,
            seed: 0,
        },
        EvaluationSet::Stratified {
            of: Box::new(EvaluationSet::Partition(secret.clone())),
            k: 1,
            by: leaven_core::Tag("kind".into()),
            seed: 0,
        },
        EvaluationSet::Union(vec![
            EvaluationSet::Partition(public.clone()),
            EvaluationSet::Partition(secret.clone()),
        ]),
        EvaluationSet::Intersect(vec![
            EvaluationSet::Partition(public),
            EvaluationSet::Partition(secret.clone()),
        ]),
        EvaluationSet::Difference(
            Box::new(EvaluationSet::Partition(secret.clone())),
            Box::new(EvaluationSet::Unscoped),
        ),
    ] {
        assert_hidden_partition_refusal(
            policy.check_evaluation_request(
                &Actor::Optimizer,
                &independent(set, EvaluationPurpose::Search),
            ),
            &secret,
        );
    }
}

#[test]
fn final_test_purpose_may_reference_optimizer_hidden_partitions() {
    let test = PartitionId::from("TEST");
    let policy = TrustPolicy::default().hide_from_optimizers([test.clone()]);
    let request = independent(
        EvaluationSet::Partition(test),
        EvaluationPurpose::FinalTest,
    );

    assert!(
        policy
            .check_evaluation_request(&Actor::Optimizer, &request)
            .is_ok()
    );
}

#[test]
fn explicit_case_ids_in_hidden_partitions_are_rejected_after_resolution() {
    let secret = PartitionId::from("secret");
    let public = PartitionId::from("public");
    let secret_case = CaseId::from_index(1);
    let public_case = CaseId::from_index(0);
    let case_set = CaseSet::new(vec!["public", "secret"])
        .with_partition(public, vec![public_case])
        .with_partition(secret.clone(), vec![secret_case]);
    let policy = TrustPolicy::default().hide_from_optimizers([secret.clone()]);

    // Expression-level check still cannot see Cases membership.
    assert!(
        policy
            .check_evaluation_request(
                &Actor::Optimizer,
                &independent(
                    EvaluationSet::Cases(vec![secret_case]),
                    EvaluationPurpose::Search
                )
            )
            .is_ok()
    );

    // Resolved membership closes the bypass.
    assert_hidden_partition_refusal(
        policy.check_resolved_cases(
            &Actor::Optimizer,
            EvaluationPurpose::Search,
            &[secret_case],
            &case_set,
        ),
        &secret,
    );
    assert!(
        policy
            .check_resolved_cases(
                &Actor::Optimizer,
                EvaluationPurpose::Search,
                &[public_case],
                &case_set
            )
            .is_ok()
    );
    assert!(
        policy
            .check_resolved_cases(
                &Actor::Optimizer,
                EvaluationPurpose::FinalTest,
                &[secret_case],
                &case_set
            )
            .is_ok()
    );
}

fn independent(set: EvaluationSet, purpose: EvaluationPurpose) -> EvaluationRequest {
    EvaluationRequest::Independent {
        candidates: vec![CandidateId::new()],
        set,
        granularity: AssessmentGranularity::Aggregate,
        purpose,
    }
}

fn pairwise(set: EvaluationSet) -> EvaluationRequest {
    EvaluationRequest::Pairwise {
        left: CandidateId::new(),
        right: CandidateId::new(),
        order: PairOrder::Ordered,
        set,
        granularity: AssessmentGranularity::Aggregate,
        purpose: EvaluationPurpose::Search,
    }
}

fn assert_hidden_partition_refusal(
    result: Result<(), leaven_engine::TrustViolation>,
    expected: &PartitionId,
) {
    let Err(leaven_engine::TrustViolation::HiddenEvaluationPartitions { partitions, .. }) = result
    else {
        panic!("expected hidden partition refusal");
    };
    assert_eq!(partitions, vec![expected.clone()]);
}

#[test]
fn unscoped_and_public_case_sets_remain_allowed() {
    let policy = TrustPolicy::default().hide_from_optimizers([PartitionId::from("secret")]);

    assert!(
        policy
            .check_evaluation_request(
                &Actor::Optimizer,
                &independent(EvaluationSet::Unscoped, EvaluationPurpose::Search)
            )
            .is_ok()
    );
    assert!(
        policy
            .check_evaluation_request(&Actor::Optimizer, &pairwise(EvaluationSet::Unscoped))
            .is_ok()
    );
}
