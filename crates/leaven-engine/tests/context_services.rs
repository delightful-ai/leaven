mod support;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use futures::executor::block_on;
use leaven_core::{
    Assessment, AssessmentGranularity, AssessmentTarget, CacheIdentity, CaseSetVersion,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, PairOrder, PartitionId, Proposal,
    ProposalBatch, ProposalBatchSemantics, ResolvedEvaluationRequest,
};
use leaven_engine::{
    BudgetLedger, CacheBypassReason, CachePolicy, CacheStatus, Callback, CaseSet,
    EvaluationCacheKey, EvaluationContext, EvaluationError, Evaluator, ProposalContext,
    ProposalError, Proposer, RunContext, RunEvent, RunGraphView, TrustPolicy,
};
use leaven_kernel::{
    AssessmentId, Budget, CaseId, ContentId, Cost, ErrorKind, EvaluatorId, Fingerprint,
    MetadataBag, Metered, ProposerId, RunId, StageAttemptFailure, StageAttemptOutcome,
    StageAttemptReceiptId, StageAttemptReceiptRef, StageId, StageRole,
};
use leaven_store::{EvidenceStore, StoreError};
use leaven_store_inline::InlineEvidenceStore;

use support::{TestEvidence, TestProblem, TextArtifact, graph_and_budget};

#[test]
fn case_reads_installed_case_set_and_is_none_without_one() {
    let (mut graph, mut budget) = graph_and_budget();
    let case_set = CaseSet::new(vec!["alpha", "beta"]);

    let bare = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    assert_eq!(bare.case(CaseId::from_index(0)), None);
    drop(bare);

    let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget).with_case_set(&case_set);
    assert_eq!(ctx.case(CaseId::from_index(0)), Some(&"alpha"));
    assert_eq!(ctx.case(CaseId::from_index(1)), Some(&"beta"));
    assert_eq!(ctx.case(CaseId::new(42)), None);
}

#[test]
fn propose_records_batch_charges_budget_and_emits_events() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let proposer = OneProposalProposer;

        let report = ctx.propose(&proposer, "x").await.unwrap();

        assert_eq!(report.proposal_ids.len(), 1);
        assert_eq!(ctx.budget().spent.llm_calls, 3);
        let events = ctx.graph().events().collect::<Vec<_>>();
        assert!(matches!(
            events.as_slice(),
            [
                RunEvent::BudgetCharged { .. },
                RunEvent::ProposalBatchProduced { .. },
                RunEvent::ProposalRecorded { .. },
            ]
        ));
    });
}

#[test]
fn stage_attempt_recorded_on_success_before_batch_events() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let receipt = StageAttemptReceiptRef {
            id: StageAttemptReceiptId::new(),
            fingerprint: Some(Fingerprint::from_bytes([17; 32])),
        };
        let proposer = StageAttemptProposer {
            receipt: receipt.clone(),
            outcome: StageAttemptOutcome::Completed,
            fail: false,
        };

        let report = ctx.propose(&proposer, ()).await.unwrap();

        assert_eq!(report.proposal_ids.len(), 1);
        let events = ctx.graph().events().collect::<Vec<_>>();
        assert!(matches!(
            events.as_slice(),
            [
                RunEvent::StageAttemptRecorded {
                    role,
                    receipt: event_receipt,
                    outcome: StageAttemptOutcome::Completed,
                    ..
                },
                RunEvent::BudgetCharged { .. },
                RunEvent::ProposalBatchProduced { .. },
                RunEvent::ProposalRecorded { .. },
            ] if role == &StageRole::reflect() && event_receipt == &receipt
        ));
    });
}

#[test]
fn stage_attempt_recorded_on_proposer_error_before_error_event() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let receipt = StageAttemptReceiptRef {
            id: StageAttemptReceiptId::new(),
            fingerprint: None,
        };
        let outcome = StageAttemptOutcome::Failed(StageAttemptFailure::OutputParse);
        let proposer = StageAttemptProposer {
            receipt: receipt.clone(),
            outcome: outcome.clone(),
            fail: true,
        };

        let error = ctx.propose(&proposer, ()).await.unwrap_err();

        assert!(error.to_string().contains("proposal failed"));
        let events = ctx.graph().events().collect::<Vec<_>>();
        assert!(matches!(
            events.as_slice(),
            [
                RunEvent::StageAttemptRecorded {
                    role,
                    receipt: event_receipt,
                    outcome: event_outcome,
                    ..
                },
                RunEvent::Error { .. },
            ] if role == &StageRole::reflect()
                && event_receipt == &receipt
                && event_outcome == &outcome
        ));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, RunEvent::ApplyFailed { .. }))
        );
    });
}

#[test]
fn proposal_context_exposes_read_scope_graph_and_budget_snapshot() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let hidden = PartitionId::from("hidden");
        {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abc".to_owned()), 0)
                .unwrap();
        }
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_trust_policy(TrustPolicy::default().hide_from_proposers([hidden.clone()]));
        let proposer = InspectingProposer { hidden };

        let report = ctx.propose(&proposer, ()).await.unwrap();

        assert_eq!(report.cost.metric_calls, 0);
        assert_eq!(ctx.budget().spent.llm_calls, 0);
    });
}

#[test]
fn render_context_exposes_renderer_scope_graph_and_budget_snapshot() {
    let (mut graph, mut budget) = graph_and_budget();
    let candidate = {
        let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        seed_ctx
            .insert_seed(TextArtifact("abc".to_owned()), 0)
            .unwrap()
    };
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget).with_trust_policy(
        TrustPolicy::default().hide_from_optimizers([PartitionId::from("optimizer-hidden")]),
    );

    let render_ctx = ctx.render_context(StageId::custom("renderer"));

    assert_eq!(
        render_ctx.graph().candidate(candidate).unwrap().id(),
        candidate
    );
    assert!(render_ctx.read_scope().hidden_partitions.is_empty());
    assert!(render_ctx.budget().spent.is_zero());
}

#[test]
fn proposer_error_records_stage_error_without_proposal_mutation() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let proposer = FailingProposer;

        let err = ctx.propose(&proposer, ()).await.unwrap_err();

        assert!(matches!(err, leaven_engine::RunContextError::Proposal(_)));
        assert_eq!(ctx.graph().proposal_batch_count(), 0);
        let error = ctx
            .graph()
            .events()
            .find_map(|event| match event {
                RunEvent::Error {
                    stage: Some(StageId::Proposer(_)),
                    error,
                    ..
                } if error.kind == ErrorKind::Proposal => Some(error),
                _ => None,
            })
            .unwrap();
        assert!(error.debug.as_deref().unwrap().contains("WithSource"));
        assert_eq!(error.source_chain, vec!["proposer backend offline"]);
    });
}

#[test]
fn evaluate_with_resolves_sets_stores_evidence_and_emits_events() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let case_set = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abc".to_owned()), 0)
                .unwrap()
        };
        let evaluator = CountingEvaluator::new(CachePolicy::Never);
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_cache(&mut cache)
            .with_evidence_store(&store);

        let report = ctx
            .evaluate_with(&evaluator, independent_request(candidate))
            .await
            .unwrap();

        assert_eq!(
            report.cache,
            CacheStatus::Bypassed(CacheBypassReason::DisabledByPolicy)
        );
        assert_eq!(report.assessment_ids.len(), 1);
        assert_eq!(evaluator.calls(), 1);
        let assessment = ctx.graph().assessment(report.assessment_ids[0]).unwrap();
        let query = ctx.graph().assessments(candidate);
        let request = ctx.graph().evaluation_request(report.request_id).unwrap();
        assert_eq!(assessment.id(), report.assessment_ids[0]);
        assert_eq!(assessment.request_id(), report.request_id);
        assert_eq!(*assessment.evaluator(), EvaluatorId::PRIMARY);
        assert!(assessment.metadata().is_empty());
        assert!(assessment.created_at() <= leaven_kernel::now());
        assert_eq!(query.len(), 1);
        assert_eq!(query.ids(), report.assessment_ids);
        assert_eq!(
            query
                .iter()
                .map(leaven_engine::AssessmentView::id)
                .collect::<Vec<_>>(),
            report.assessment_ids
        );
        assert_eq!(*request.evaluator(), EvaluatorId::PRIMARY);
        assert_eq!(request.id(), report.request_id);
        assert!(matches!(
            request.request(),
            EvaluationRequest::Independent { .. }
        ));
        assert_eq!(request.resolved_set().case_ids.len(), 1);
        assert!(request.created_at() <= leaven_kernel::now());
        assert!(matches!(assessment.target(), AssessmentTarget::Unscoped));
        assert_eq!(assessment.independent_candidate(), Some(candidate));
        assert_score(
            ctx.assessment_evidence(report.assessment_ids[0])
                .unwrap()
                .score,
            3.0,
        );
        let evidence = store
            .get(assessment.evidence_ref())
            .map_err(|err| match err {
                StoreError::EvidenceNotFound(_) => "missing evidence",
                _ => "store failure",
            })
            .unwrap();
        assert_score(evidence.score, 3.0);
        assert!(
            ctx.graph()
                .events()
                .any(|event| matches!(event, RunEvent::EvaluationRequested { .. }))
        );
        assert!(ctx.graph().events().any(|event| matches!(
            event,
            RunEvent::EvaluationCompleted {
                cache: CacheStatus::Bypassed(CacheBypassReason::DisabledByPolicy),
                ..
            }
        )));
    });
}

#[test]
fn pairwise_and_listwise_evaluations_record_non_independent_targets() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let case_set = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let candidates = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            vec![
                seed_ctx
                    .insert_seed(TextArtifact("a".to_owned()), 0)
                    .unwrap(),
                seed_ctx
                    .insert_seed(TextArtifact("b".to_owned()), 1)
                    .unwrap(),
                seed_ctx
                    .insert_seed(TextArtifact("c".to_owned()), 2)
                    .unwrap(),
            ]
        };
        let evaluator = ShapeEvaluator;
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_cache(&mut cache)
            .with_evidence_store(&store);

        let pairwise = ctx
            .evaluate_with(
                &evaluator,
                EvaluationRequest::Pairwise {
                    left: candidates[0],
                    right: candidates[1],
                    order: PairOrder::Ordered,
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::Selection,
                },
            )
            .await
            .unwrap();
        let listwise = ctx
            .evaluate_with(
                &evaluator,
                EvaluationRequest::Listwise {
                    candidates: candidates.clone(),
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::Selection,
                },
            )
            .await
            .unwrap();

        let pair_assessment = ctx.graph().assessment(pairwise.assessment_ids[0]).unwrap();
        let list_assessment = ctx.graph().assessment(listwise.assessment_ids[0]).unwrap();
        let pair_query = ctx
            .graph()
            .pairwise_assessments(candidates[0], candidates[1]);
        let reversed_pair_query = ctx
            .graph()
            .pairwise_assessments(candidates[1], candidates[0]);
        let list_query = ctx.graph().assessments(candidates[2]);
        assert_eq!(pair_assessment.independent_candidate(), None);
        assert_eq!(list_assessment.independent_candidate(), None);
        assert!(matches!(
            pair_assessment.target(),
            AssessmentTarget::Unscoped
        ));
        assert!(matches!(
            list_assessment.target(),
            AssessmentTarget::Unscoped
        ));
        assert_eq!(
            pair_assessment.pairwise_candidates(),
            Some((candidates[0], candidates[1]))
        );
        assert_eq!(pair_assessment.listwise_candidates(), None);
        assert_eq!(list_assessment.pairwise_candidates(), None);
        assert_eq!(
            list_assessment.listwise_candidates(),
            Some(candidates.as_slice())
        );
        assert_eq!(pair_query.ids(), pairwise.assessment_ids);
        assert!(reversed_pair_query.is_empty());
        assert_eq!(list_query.ids(), listwise.assessment_ids);
        assert_score(
            ctx.assessment_evidence(pairwise.assessment_ids[0])
                .unwrap()
                .score,
            2.0,
        );
        assert_score(
            ctx.assessment_evidence(listwise.assessment_ids[0])
                .unwrap()
                .score,
            3.0,
        );
    });
}

#[test]
fn evaluation_error_records_request_and_stage_error_without_assessment_mutation() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let case_set = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abc".to_owned()), 0)
                .unwrap()
        };
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_cache(&mut cache)
            .with_evidence_store(&store);

        let err = ctx
            .evaluate_with(&FailingEvaluator, independent_request(candidate))
            .await
            .unwrap_err();

        assert!(matches!(err, leaven_engine::RunContextError::Evaluation(_)));
        assert_eq!(ctx.graph().evaluation_request_count(), 1);
        assert!(
            ctx.graph()
                .events()
                .any(|event| matches!(event, RunEvent::EvaluationRequested { .. }))
        );
        assert_eq!(ctx.graph().assessment_count(), 0);
        let error = ctx
            .graph()
            .events()
            .find_map(|event| match event {
                RunEvent::Error {
                    stage: Some(StageId::Evaluator(_)),
                    error,
                    ..
                } if error.kind == ErrorKind::Evaluation => Some(error),
                _ => None,
            })
            .unwrap();
        assert!(error.debug.as_deref().unwrap().contains("WithSource"));
        assert_eq!(error.source_chain, vec!["metric backend offline"]);
    });
}

#[test]
fn metered_evaluation_errors_charge_budget_before_error_return() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let case_set = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abc".to_owned()), 0)
                .unwrap()
        };
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_cache(&mut cache)
            .with_evidence_store(&store);

        let err = ctx
            .evaluate_with(&CostedFailingEvaluator, independent_request(candidate))
            .await
            .unwrap_err();

        assert!(matches!(err, leaven_engine::RunContextError::Evaluation(_)));
        assert_eq!(ctx.budget().spent.llm_calls, 2);
        assert_eq!(ctx.graph().assessment_count(), 0);
        assert!(ctx.graph().events().any(|event| matches!(
            event,
            RunEvent::BudgetCharged {
                stage: StageId::Evaluator(_),
                ..
            }
        )));
    });
}

#[test]
fn evidence_store_error_records_stage_error_after_request_without_assessment_mutation() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let case_set = CaseSet::new(vec!["case"]);
        let store = RejectingEvidenceStore;
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abc".to_owned()), 0)
                .unwrap()
        };
        let evaluator = CountingEvaluator::new(CachePolicy::Never);
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_cache(&mut cache)
            .with_evidence_store(&store);

        let err = ctx
            .evaluate_with(&evaluator, independent_request(candidate))
            .await
            .unwrap_err();

        assert!(matches!(err, leaven_engine::RunContextError::Store(_)));
        assert_eq!(ctx.graph().evaluation_request_count(), 1);
        assert_eq!(ctx.graph().assessment_count(), 0);
        assert!(
            !ctx.graph()
                .events()
                .any(|event| matches!(event, RunEvent::EvaluationCompleted { .. }))
        );
        let error = ctx
            .graph()
            .events()
            .find_map(|event| match event {
                RunEvent::Error {
                    stage: Some(StageId::Evaluator(_)),
                    error,
                    ..
                } if error.kind == ErrorKind::Store => Some(error),
                _ => None,
            })
            .unwrap();
        assert!(error.message.contains("refused put_evidence"));
        assert!(error.debug.as_deref().unwrap().contains("OperationFailed"));
    });
}

#[test]
fn evaluation_requires_case_set_and_evidence_store() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let case_set = CaseSet::new(vec!["case"]);
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abc".to_owned()), 0)
                .unwrap()
        };
        let evaluator = CountingEvaluator::new(CachePolicy::Never);

        let missing_case_set = {
            let mut ctx =
                RunContext::<TestProblem>::new(&mut graph, &mut budget).with_cache(&mut cache);
            ctx.evaluate_with(&evaluator, independent_request(candidate))
                .await
                .unwrap_err()
        };
        let missing_store = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_cache(&mut cache);
            ctx.evaluate_with(&evaluator, independent_request(candidate))
                .await
                .unwrap_err()
        };
        let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);

        assert!(matches!(
            missing_case_set,
            leaven_engine::RunContextError::MissingCaseSet
        ));
        assert!(matches!(
            missing_store,
            leaven_engine::RunContextError::MissingEvidenceStore
        ));
        assert!(matches!(
            ctx.assessment_evidence(leaven_kernel::AssessmentId::new()),
            Err(leaven_engine::RunContextError::UnknownAssessment(_))
        ));
    });
}

#[test]
fn deterministic_evaluation_cache_skips_second_evaluator_call() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let case_set = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abcd".to_owned()), 0)
                .unwrap()
        };
        let evaluator = CountingEvaluator::new(CachePolicy::Deterministic);

        let first = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_cache(&mut cache)
                .with_evidence_store(&store);
            ctx.evaluate_with(&evaluator, independent_request(candidate))
                .await
                .unwrap()
        };
        let second = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_cache(&mut cache)
                .with_evidence_store(&store);
            ctx.evaluate_with(&evaluator, independent_request(candidate))
                .await
                .unwrap()
        };

        assert_eq!(first.cache, CacheStatus::Miss);
        assert_eq!(second.cache, CacheStatus::Hit);
        assert_eq!(first.assessment_ids, second.assessment_ids);
        assert_eq!(evaluator.calls(), 1);
    });
}

#[test]
fn deterministic_evaluation_ignores_cache_entries_with_missing_graph_assessments() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let case_set = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abcd".to_owned()), 0)
                .unwrap()
        };
        let mut content = [0; 32];
        content[..4].copy_from_slice(b"abcd");
        cache.insert(
            EvaluationCacheKey {
                evaluator: Fingerprint::from_bytes([7; 32]),
                policy: CachePolicy::Deterministic,
                case_set_version: CaseSetVersion("0".to_owned()),
                case_ids: vec![leaven_kernel::CaseId::from_index(0)],
                candidates: vec![CacheIdentity::Content(ContentId::from_bytes(content))],
            },
            vec![AssessmentId::new()],
        );
        let evaluator = CountingEvaluator::new(CachePolicy::Deterministic);

        let report = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_cache(&mut cache)
                .with_evidence_store(&store);
            ctx.evaluate_with(&evaluator, independent_request(candidate))
                .await
                .unwrap()
        };

        assert_eq!(report.cache, CacheStatus::Miss);
        assert_eq!(report.assessment_ids.len(), 1);
        assert_eq!(evaluator.calls(), 1);
    });
}

#[test]
fn deterministic_evaluation_without_cache_store_reports_unavailable_bypass() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let case_set = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abcd".to_owned()), 0)
                .unwrap()
        };
        let evaluator = CountingEvaluator::new(CachePolicy::Deterministic);

        let first = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
            ctx.evaluate_with(&evaluator, independent_request(candidate))
                .await
                .unwrap()
        };
        let second = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
            ctx.evaluate_with(&evaluator, independent_request(candidate))
                .await
                .unwrap()
        };

        assert_eq!(
            first.cache,
            CacheStatus::Bypassed(CacheBypassReason::CacheUnavailable)
        );
        assert_eq!(
            second.cache,
            CacheStatus::Bypassed(CacheBypassReason::CacheUnavailable)
        );
        assert_ne!(first.assessment_ids, second.assessment_ids);
        assert_eq!(evaluator.calls(), 2);
    });
}

#[test]
fn deterministic_evaluation_cache_bypasses_external_artifacts_without_cache_identity() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let case_set = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("external:branch-main".to_owned()), 0)
                .unwrap()
        };
        let evaluator = CountingEvaluator::new(CachePolicy::Deterministic);

        let first = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_cache(&mut cache)
                .with_evidence_store(&store);
            ctx.evaluate_with(&evaluator, independent_request(candidate))
                .await
                .unwrap()
        };
        let second = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_cache(&mut cache)
                .with_evidence_store(&store);
            ctx.evaluate_with(&evaluator, independent_request(candidate))
                .await
                .unwrap()
        };

        assert_eq!(
            first.cache,
            CacheStatus::Bypassed(CacheBypassReason::MissingCandidateIdentity { candidate })
        );
        assert_eq!(
            second.cache,
            CacheStatus::Bypassed(CacheBypassReason::MissingCandidateIdentity { candidate })
        );
        assert_ne!(first.assessment_ids, second.assessment_ids);
        assert_eq!(evaluator.calls(), 2);
    });
}

#[test]
fn no_cache_policy_invokes_evaluator_and_records_each_request() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let case_set = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abc".to_owned()), 0)
                .unwrap()
        };
        let evaluator = CountingEvaluator::new(CachePolicy::Never);

        let first = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_cache(&mut cache)
                .with_evidence_store(&store);
            ctx.evaluate_with(&evaluator, independent_request(candidate))
                .await
                .unwrap()
        };
        let second = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_cache(&mut cache)
                .with_evidence_store(&store);
            ctx.evaluate_with(&evaluator, independent_request(candidate))
                .await
                .unwrap()
        };
        let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);

        assert_eq!(
            first.cache,
            CacheStatus::Bypassed(CacheBypassReason::DisabledByPolicy)
        );
        assert_eq!(
            second.cache,
            CacheStatus::Bypassed(CacheBypassReason::DisabledByPolicy)
        );
        assert_ne!(first.assessment_ids, second.assessment_ids);
        assert_eq!(evaluator.calls(), 2);
        assert_eq!(ctx.graph().evaluation_request_count(), 2);
    });
}

#[test]
fn propose_budget_exhaustion_leaves_graph_unmutated() {
    block_on(async {
        let mut graph = leaven_engine::RunGraph::new(leaven_kernel::RunId::new());
        let mut budget = BudgetLedger::new(Budget {
            llm_calls: Some(2),
            ..Budget::unlimited()
        });
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let proposer = OneProposalProposer;

        let err = ctx.propose(&proposer, "x").await.unwrap_err();

        assert!(matches!(err, leaven_engine::RunContextError::Budget(_)));
        assert_eq!(ctx.graph().proposal_batch_count(), 0);
        assert_eq!(ctx.graph().proposal_count(), 0);
        assert!(ctx.graph().events().any(|event| matches!(
            event,
            RunEvent::Error {
                stage: Some(StageId::Proposer(_)),
                ..
            }
        )));
    });
}

#[test]
fn evaluate_budget_exhaustion_records_request_without_assessment_mutation() {
    block_on(async {
        let mut graph = leaven_engine::RunGraph::new(leaven_kernel::RunId::new());
        let mut budget = BudgetLedger::new(Budget::metric_calls(0));
        let mut cache = leaven_engine::EvaluationCache::default();
        let case_set = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abc".to_owned()), 0)
                .unwrap()
        };
        let evaluator = CountingEvaluator::new(CachePolicy::Never);
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_cache(&mut cache)
            .with_evidence_store(&store);

        let err = ctx
            .evaluate_with(&evaluator, independent_request(candidate))
            .await
            .unwrap_err();

        assert!(matches!(err, leaven_engine::RunContextError::Budget(_)));
        assert_eq!(evaluator.calls(), 1);
        assert_eq!(ctx.graph().evaluation_request_count(), 1);
        assert_eq!(ctx.graph().assessment_count(), 0);
        assert!(ctx.graph().events().any(|event| matches!(
            event,
            RunEvent::Error {
                stage: Some(StageId::Evaluator(_)),
                ..
            }
        )));
    });
}

#[test]
fn read_scope_hides_assessments_from_forbidden_partitions() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let test_partition = PartitionId::from("TEST");
        let case_set = CaseSet::new(vec!["case"]).with_partition(
            test_partition.clone(),
            vec![leaven_kernel::CaseId::from_index(0)],
        );
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abc".to_owned()), 0)
                .unwrap()
        };
        let evaluator = CountingEvaluator::new(CachePolicy::Never);
        let report = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_cache(&mut cache)
                .with_evidence_store(&store);
            ctx.evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::Partition(test_partition.clone()),
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::FinalTest,
                },
            )
            .await
            .unwrap()
        };
        let assessment_id = report.assessment_ids[0];

        let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_trust_policy(TrustPolicy::default().hide_from_optimizers([test_partition]));

        assert!(ctx.graph().assessment(assessment_id).is_none());
        assert!(ctx.graph().assessments(candidate).is_empty());
        assert!(ctx.graph().evaluation_request(report.request_id).is_none());
    });
}

#[test]
fn stage_engine_context_uses_scoped_graph_without_exposing_raw_view() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let test_partition = PartitionId::from("TEST");
        let case_set = CaseSet::new(vec!["case"]).with_partition(
            test_partition.clone(),
            vec![leaven_kernel::CaseId::from_index(0)],
        );
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abc".to_owned()), 0)
                .unwrap()
        };
        let evaluator = CountingEvaluator::new(CachePolicy::Never);
        let report = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_cache(&mut cache)
                .with_evidence_store(&store);
            ctx.evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::Partition(test_partition.clone()),
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::FinalTest,
                },
            )
            .await
            .unwrap()
        };

        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_trust_policy(TrustPolicy::default().hide_from_proposers([test_partition]));
        let proposal_ctx = ctx.proposal_context(StageId::from_proposer(ProposerId::from("stage")));
        let stage_ctx = proposal_ctx.stage_engine_context();

        assert_eq!(stage_ctx.stage_call_id(), proposal_ctx.stage_call_id());
        assert_eq!(
            stage_ctx.graph().candidate(candidate).unwrap().id(),
            candidate
        );
        assert_eq!(stage_ctx.graph().artifact(candidate).unwrap().0, "abc");
        assert!(
            stage_ctx
                .graph()
                .assessment(report.assessment_ids[0])
                .is_none()
        );
        assert!(
            stage_ctx
                .graph()
                .assessments_for_candidate(candidate)
                .is_empty()
        );
    });
}

#[test]
fn hidden_partition_evaluation_request_records_trust_violation_without_mutation() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let secret = PartitionId::from("secret");
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abc".to_owned()), 0)
                .unwrap()
        };
        let evaluator = CountingEvaluator::new(CachePolicy::Never);
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_cache(&mut cache)
            .with_evidence_store(&store)
            .with_trust_policy(TrustPolicy::default().hide_from_optimizers([secret.clone()]));

        let err = ctx
            .evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::Partition(secret.clone()),
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            leaven_engine::RunContextError::TrustViolation(
                leaven_engine::TrustViolation::HiddenEvaluationPartitions { .. }
            )
        ));
        assert_eq!(evaluator.calls(), 0);
        assert_eq!(ctx.graph().evaluation_request_count(), 0);
        assert_eq!(ctx.graph().assessment_count(), 0);
        assert!(ctx.graph().events().any(|event| matches!(
            event,
            RunEvent::Error {
                error,
                ..
            } if error.kind == ErrorKind::Trust
        )));
    });
}

#[test]
fn callbacks_receive_callback_read_scope() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let secret = PartitionId::from("secret");
        let case_set = CaseSet::new(vec!["case"])
            .with_partition(secret.clone(), vec![leaven_kernel::CaseId::from_index(0)]);
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abc".to_owned()), 0)
                .unwrap()
        };
        let assessment_id = {
            let evaluator = CountingEvaluator::new(CachePolicy::Never);
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_cache(&mut cache)
                .with_evidence_store(&store);
            ctx.evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::Partition(secret.clone()),
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::FinalTest,
                },
            )
            .await
            .unwrap()
            .assessment_ids[0]
        };
        let hidden_observations = Arc::new(AtomicUsize::new(0));
        let visible_observations = Arc::new(AtomicUsize::new(0));
        let callback = VisibilityCallback {
            assessment_id,
            hidden_partition: secret.clone(),
            hidden_observations: hidden_observations.clone(),
            visible_observations: visible_observations.clone(),
        };
        let mut callbacks: Vec<Box<dyn leaven_engine::DynCallback<TestProblem>>> =
            vec![Box::new(callback)];
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_trust_policy(TrustPolicy::default().hide_from_callbacks([secret]))
            .with_callbacks(callbacks.as_mut_slice());

        ctx.emit(RunEvent::OptimizationStarted {
            run_id: RunId::new(),
        });

        assert_eq!(hidden_observations.load(Ordering::SeqCst), 1);
        assert_eq!(visible_observations.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn read_scope_hides_nested_assessment_sets_that_reference_forbidden_partitions() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let secret = PartitionId::from("secret");
        let public = PartitionId::from("public");
        let case_set = CaseSet::new(vec!["a", "b"])
            .with_partition(secret.clone(), vec![leaven_kernel::CaseId::new(0)])
            .with_partition(public.clone(), vec![leaven_kernel::CaseId::new(1)]);
        let candidate = {
            let mut seed_ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            seed_ctx
                .insert_seed(TextArtifact("abc".to_owned()), 0)
                .unwrap()
        };
        let evaluator = CountingEvaluator::new(CachePolicy::Never);
        let mut assessment_ids = Vec::new();
        {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_cache(&mut cache)
                .with_evidence_store(&store);
            for set in [
                EvaluationSet::All,
                EvaluationSet::Sample {
                    of: Box::new(EvaluationSet::Partition(secret.clone())),
                    n: 1,
                    seed: 0,
                },
                EvaluationSet::Union(vec![
                    EvaluationSet::Partition(public),
                    EvaluationSet::Partition(secret.clone()),
                ]),
                EvaluationSet::Difference(
                    Box::new(EvaluationSet::Partition(secret.clone())),
                    Box::new(EvaluationSet::Unscoped),
                ),
                EvaluationSet::Cases(vec![leaven_kernel::CaseId::new(1)]),
            ] {
                let report = ctx
                    .evaluate_with(
                        &evaluator,
                        EvaluationRequest::Independent {
                            candidates: vec![candidate],
                            set,
                            granularity: AssessmentGranularity::Aggregate,
                            purpose: EvaluationPurpose::FinalTest,
                        },
                    )
                    .await
                    .unwrap();
                assessment_ids.push(report.assessment_ids[0]);
            }
        }

        let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_trust_policy(TrustPolicy::default().hide_from_optimizers([secret]));

        assert!(ctx.graph().assessment(assessment_ids[0]).is_none());
        assert!(ctx.graph().assessment(assessment_ids[1]).is_none());
        assert!(ctx.graph().assessment(assessment_ids[2]).is_none());
        assert!(ctx.graph().assessment(assessment_ids[3]).is_none());
        assert!(ctx.graph().assessment(assessment_ids[4]).is_some());
    });
}

struct InspectingProposer {
    hidden: PartitionId,
}

struct VisibilityCallback {
    assessment_id: leaven_kernel::AssessmentId,
    hidden_partition: PartitionId,
    hidden_observations: Arc<AtomicUsize>,
    visible_observations: Arc<AtomicUsize>,
}

impl Callback<TestProblem> for VisibilityCallback {
    fn on_event(&mut self, _event: &RunEvent, graph: RunGraphView<'_, TestProblem>) {
        assert!(
            graph
                .read_scope()
                .hidden_partitions
                .contains(&self.hidden_partition)
        );
        if graph.assessment(self.assessment_id).is_none() {
            self.hidden_observations.fetch_add(1, Ordering::SeqCst);
        } else {
            self.visible_observations.fetch_add(1, Ordering::SeqCst);
        }
    }
}

impl Proposer<TestProblem> for InspectingProposer {
    type Request = ();

    fn id(&self) -> ProposerId {
        ProposerId::from("inspect")
    }

    async fn propose(
        &self,
        _request: Self::Request,
        ctx: ProposalContext<'_, TestProblem>,
    ) -> Result<Metered<ProposalBatch<TestProblem>>, ProposalError> {
        assert_eq!(ctx.graph().candidate_count(), 1);
        assert!(ctx.read_scope().hidden_partitions.contains(&self.hidden));
        assert!(ctx.budget().spent.is_zero());
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

struct OneProposalProposer;

impl Proposer<TestProblem> for OneProposalProposer {
    type Request = &'static str;

    fn id(&self) -> ProposerId {
        ProposerId::from("one")
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
            Cost::llm_calls(3),
        ))
    }
}

struct StageAttemptProposer {
    receipt: StageAttemptReceiptRef,
    outcome: StageAttemptOutcome,
    fail: bool,
}

impl Proposer<TestProblem> for StageAttemptProposer {
    type Request = ();

    fn id(&self) -> ProposerId {
        ProposerId::from("stage-attempt")
    }

    async fn propose(
        &self,
        _request: Self::Request,
        ctx: ProposalContext<'_, TestProblem>,
    ) -> Result<Metered<ProposalBatch<TestProblem>>, ProposalError> {
        ctx.record_stage_attempt(
            StageRole::reflect(),
            self.receipt.clone(),
            self.outcome.clone(),
        );
        if self.fail {
            return Err(ProposalError::Message("parse failed".to_owned()));
        }
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![Proposal::create(TextArtifact("stage".to_owned())).build()],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        ))
    }
}

struct FailingProposer;

impl Proposer<TestProblem> for FailingProposer {
    type Request = ();

    fn id(&self) -> ProposerId {
        ProposerId::from("fail")
    }

    async fn propose(
        &self,
        _request: Self::Request,
        _ctx: ProposalContext<'_, TestProblem>,
    ) -> Result<Metered<ProposalBatch<TestProblem>>, ProposalError> {
        Err(ProposalError::with_source(
            "no proposal",
            StaticTestError("proposer backend offline"),
        ))
    }
}

struct CountingEvaluator {
    calls: Arc<AtomicUsize>,
    cache_policy: CachePolicy,
}

impl CountingEvaluator {
    fn new(cache_policy: CachePolicy) -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            cache_policy,
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

struct ShapeEvaluator;

impl Evaluator<TestProblem> for ShapeEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::from("shape")
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
        mut ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        assert!(ctx.graph().candidate_count() >= 2);
        assert!(ctx.read_scope().hidden_partitions.is_empty());
        assert!(ctx.budget().spent.metric_calls <= 1);
        let _ = ctx.budget_handle().snapshot();
        let assessment = match request.kind {
            leaven_core::ResolvedRequestKind::Independent { candidates } => {
                Assessment::Independent {
                    candidate: candidates[0],
                    target: AssessmentTarget::Unscoped,
                    evidence: TestEvidence { score: 1.0 },
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                }
            }
            leaven_core::ResolvedRequestKind::Pairwise { left, right, .. } => {
                Assessment::Pairwise {
                    left,
                    right,
                    target: AssessmentTarget::Unscoped,
                    evidence: TestEvidence { score: 2.0 },
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                }
            }
            leaven_core::ResolvedRequestKind::Listwise { candidates } => Assessment::Listwise {
                candidates,
                target: AssessmentTarget::Unscoped,
                evidence: TestEvidence { score: 3.0 },
                cost: Cost::metric_calls(1),
                metadata: MetadataBag::new(),
            },
        };
        Ok(Metered::new(vec![assessment], Cost::metric_calls(1)))
    }
}

struct FailingEvaluator;

impl Evaluator<TestProblem> for FailingEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::from("fail")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([9; 32])
    }

    async fn evaluate(
        &self,
        _request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        Err(EvaluationError::with_source(
            "evaluation failed",
            StaticTestError("metric backend offline"),
        ))
    }
}

struct CostedFailingEvaluator;

impl Evaluator<TestProblem> for CostedFailingEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::from("costed-fail")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([11; 32])
    }

    async fn evaluate(
        &self,
        _request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        Err(EvaluationError::with_cost_source(
            "evaluation failed after judge call",
            Cost::llm_calls(2),
            StaticTestError("judge backend offline"),
        ))
    }
}

struct RejectingEvidenceStore;

impl EvidenceStore<TestEvidence> for RejectingEvidenceStore {
    fn put(&self, _evidence: TestEvidence) -> Result<leaven_kernel::EvidenceRef, StoreError> {
        Err(StoreError::OperationFailed {
            store: "rejecting".to_owned(),
            operation: "put_evidence",
            reason: "disk full".to_owned(),
            retryable: Some(true),
        })
    }

    fn get(&self, reference: &leaven_kernel::EvidenceRef) -> Result<TestEvidence, StoreError> {
        Err(StoreError::EvidenceNotFound(reference.clone()))
    }
}

#[derive(Debug)]
struct StaticTestError(&'static str);

impl std::fmt::Display for StaticTestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for StaticTestError {}

impl Evaluator<TestProblem> for CountingEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([7; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        self.cache_policy.clone()
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let leaven_core::ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message("expected independent".to_owned()));
        };
        Ok(Metered::new(
            candidates
                .into_iter()
                .map(|candidate| Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Unscoped,
                    evidence: TestEvidence { score: 3.0 },
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                })
                .collect(),
            Cost::metric_calls(1),
        ))
    }
}

fn independent_request(candidate: leaven_kernel::CandidateId) -> EvaluationRequest {
    EvaluationRequest::Independent {
        candidates: vec![candidate],
        set: EvaluationSet::All,
        granularity: AssessmentGranularity::Aggregate,
        purpose: EvaluationPurpose::Search,
    }
}

fn assert_score(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < f64::EPSILON);
}
