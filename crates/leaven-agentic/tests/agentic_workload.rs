use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use futures::future::{BoxFuture, FutureExt};
use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, AgentRuntimeError,
    AgentSession, FakeAgentAction, FakeAgentRuntime, JsonSchemaRef, OutputContract,
};
use leaven_agentic::{
    AgentCase, AgentCaseEvaluator, AgentCaseEvaluatorConfig, AgentCasePresentation,
    AgentCasePresentationInput, AgentCasePresenter, AgentCaseRetryRecord, AgentCaseRunError,
    AgentCaseRunPolicy, AgentCaseRunRecord, AgentCaseScoreInput, AgentCaseScorer,
    AgentRunPreflight, AgentWorkload, AgenticAdapterError, AgenticInspectionWarning,
    AgenticRunInspection, CASE_RUN_RECORD_METADATA_KEY, CaseCheckpointPolicy, CaseFiles, CaseInput,
    CaseMessage, CasePartitionId, CasePartitions, CaseSuite, CaseTarget, FailOnError,
    FailedAgentCaseRun, FiniteRatio, PreflightSeverity, PresenterDryRun, ScoredAgentCaseRun,
    ScorerDryRun, SetupScript, ToolApprovalPolicy, WorkspaceRequirement,
};
use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget, CacheIdentity,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, OptimizationProblem, PairOrder,
    ResolvedEvaluationRequest,
};
use leaven_engine::{
    BudgetLedger, CacheBypassReason, CachePolicy, CacheStatus, CaseSet, EvaluationContext,
    EvaluationError, Evaluator, MaterializeContext, RunContext, RunEvent, RunGraph,
};
use leaven_kernel::{
    AgentSessionId, BlobRef, CaseId, CheckpointId, ContentId, Cost, EvaluatorId, Fingerprint,
    MetadataKey, MetadataValue, Metered, RunId, StageId,
};
use leaven_store::{BlobStore, BlobWrite, CheckpointBytes, CheckpointStore, StoreError};
use leaven_store_inline::{InlineEvidenceStore, InlineStore};
use leaven_workspace::{
    FactoryError, Workspace, WorkspaceBackend, WorkspaceError, WorkspaceFactory,
};
use leaven_workspace::{WorkspaceConfig, WorkspacePath, WorkspaceView};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn case_suite_fingerprint_changes_when_case_content_or_partitions_change() {
    let case = AgentCase::text(
        CaseId::new(0),
        "input",
        CaseTarget::Text("answer".to_owned()),
    );
    let baseline = CaseSuite::from_cases([case.clone()]).unwrap();

    let changed_case = AgentCase::text(case.id, "different", CaseTarget::Text("answer".to_owned()));
    let changed_cases = CaseSuite::from_cases([changed_case]).unwrap();

    let partitions = CasePartitions::with_all(vec![case.id])
        .with_partition(CasePartitionId::from("feedback"), vec![case.id]);
    let mut map = BTreeMap::new();
    map.insert(case.id, case);
    let changed_partitions = CaseSuite::new(map, partitions).unwrap();

    assert_ne!(baseline.fingerprint(), changed_cases.fingerprint());
    assert_ne!(baseline.fingerprint(), changed_partitions.fingerprint());
}

#[test]
fn case_suite_rejects_duplicate_ids_and_missing_partition_targets() {
    let id = CaseId::new(10);
    let one = AgentCase::text(id, "one", CaseTarget::None);
    let two = AgentCase::text(id, "two", CaseTarget::None);

    let duplicate = CaseSuite::from_cases([one.clone(), two]).unwrap_err();
    assert!(duplicate.to_string().contains("duplicate agent case id"));

    let partitions = CasePartitions::with_all(vec![CaseId::new(99)]);
    let mut cases = BTreeMap::new();
    cases.insert(id, one);

    let missing = CaseSuite::new(cases, partitions).unwrap_err();
    assert!(missing.to_string().contains("references missing case"));
}

#[test]
fn workload_from_cases_and_parts_preserve_suite_validation() {
    let first = AgentCase::text(CaseId::new(1), "first", CaseTarget::None);
    let second = AgentCase::text(CaseId::new(2), "second", CaseTarget::None);

    let workload = AgentWorkload::from_cases([first.clone(), second.clone()]).unwrap();
    assert_eq!(workload.cases().cases().len(), 2);
    assert_eq!(
        workload.partitions().named()[&CasePartitionId::all()],
        vec![first.id, second.id]
    );
    assert_eq!(workload.fingerprint(), workload.cases().fingerprint());
    assert!(!workload.is_empty());

    let duplicate = AgentWorkload::from_cases([first.clone(), first.clone()]).unwrap_err();
    assert!(duplicate.to_string().contains("duplicate agent case id"));

    let mut cases = BTreeMap::new();
    cases.insert(first.id, first.clone());
    cases.insert(second.id, second.clone());
    let partitions = CasePartitions::with_all(vec![first.id, second.id])
        .with_partition(CasePartitionId::from("train"), vec![second.id]);
    let from_parts = AgentWorkload::from_parts(cases, partitions).unwrap();
    assert_eq!(
        from_parts.partitions().named()[&CasePartitionId::from("train")],
        vec![second.id]
    );

    let mut cases = BTreeMap::new();
    cases.insert(first.id, first);
    let missing =
        AgentWorkload::from_parts(cases, CasePartitions::with_all(vec![CaseId::new(404)]))
            .unwrap_err();
    assert!(missing.to_string().contains("references missing case"));
}

#[test]
fn agent_case_vocabulary_preserves_files_messages_setup_and_workspace_requirements() {
    let mut files = CaseFiles::default();
    files.insert(
        WorkspacePath::new("input/problem.txt").unwrap(),
        b"problem".to_vec(),
    );
    assert_eq!(
        files.files()[&WorkspacePath::new("input/problem.txt").unwrap()],
        b"problem".to_vec()
    );

    let case = AgentCase {
        id: CaseId::new(22),
        input: CaseInput::Messages(vec![CaseMessage {
            role: "user".to_owned(),
            content: "Convert the fraction.".to_owned(),
        }]),
        target: CaseTarget::Structured(serde_json::json!({ "answer": "1/16" })),
        metadata: leaven_kernel::MetadataBag::new(),
        files,
        setup: Some(SetupScript {
            command: vec!["python".to_owned(), "setup.py".to_owned()],
        }),
        workspace: Some(WorkspaceRequirement::RequiresCommands),
    };
    let suite = CaseSuite::from_cases([case]).unwrap();
    let workload = AgentWorkload::new(suite);
    assert_eq!(workload.cases().cases().len(), 1);
    assert!(CaseTarget::Hidden(ContentId::from_bytes([7; 32])).is_hidden());

    let empty_partition = CasePartitionId::new("").unwrap_err();
    assert!(empty_partition.to_string().contains("cannot be empty"));
}

#[test]
fn hidden_target_is_not_presented_to_candidate() {
    futures::executor::block_on(async {
        let secret = ContentId::from_bytes([9; 32]);
        let case = AgentCase::text(CaseId::new(44), "visible input", CaseTarget::Hidden(secret));
        let presenter = TestPresenter;
        let factory = LocalWorkspaceFactory::temp();
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let candidate_id = {
            let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(CaseArtifact("candidate body".to_owned()), 0)
                .unwrap()
        };
        let ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
        let materialize_ctx = ctx.materialize_context();

        let presentation = {
            let mut view = workspace.view();
            presenter
                .present(
                    AgentCasePresentationInput {
                        candidate_id,
                        candidate: &CaseArtifact("candidate body".to_owned()),
                        case: &case,
                        graph: materialize_ctx.graph().clone(),
                    },
                    &mut view,
                    materialize_ctx,
                )
                .await
                .unwrap()
                .value
        };

        let forbidden_strings = [
            hex::encode(secret.as_bytes()),
            secret.to_string(),
            format!("{secret:?}"),
        ];
        let instructions = serde_json::to_string(&presentation.request.instructions).unwrap();
        for forbidden in &forbidden_strings {
            assert!(
                !instructions.contains(forbidden),
                "hidden target leaked into instructions as `{forbidden}`"
            );
        }

        {
            let view = workspace.view();
            for path in &presentation.materialized_refs {
                let bytes = view.read_file(path).unwrap();
                let rendered = String::from_utf8_lossy(&bytes);
                for forbidden in &forbidden_strings {
                    assert!(
                        !rendered.contains(forbidden),
                        "hidden target leaked into workspace file {} as `{forbidden}`",
                        path.as_str()
                    );
                }
                assert!(
                    !bytes
                        .windows(secret.as_bytes().len())
                        .any(|window| window == secret.as_bytes()),
                    "hidden target bytes leaked into workspace file {}",
                    path.as_str()
                );
            }
        }

        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn case_run_records_preserve_scored_failed_retry_and_policy_state() {
    let attempt = NonZeroUsize::new(1).unwrap();
    let retry_attempt = NonZeroUsize::new(2).unwrap();
    let retry = AgentCaseRetryRecord {
        attempt: retry_attempt,
        session: Some(AgentSessionId::new()),
        error: AgentCaseRunError::Runtime("first attempt failed".to_owned()),
        cost: Cost::llm_calls(1),
    };
    let scored = AgentCaseRunRecord::scored_attempt(ScoredAgentCaseRun {
        run_id: RunId::new(),
        candidate: leaven_kernel::CandidateId::new(),
        case: CaseId::new(7),
        partition: leaven_kernel::EvaluationSetId::new(),
        attempt,
        session: AgentSessionId::new(),
        outputs: vec![WorkspacePath::new("output/result.json").unwrap()],
        retries: vec![retry.clone()],
        cost: Cost::metric_calls(1),
    });

    assert!(scored.score_recorded);
    assert!(scored.error.is_none());
    assert_eq!(scored.retries, vec![retry.clone()]);
    assert!(AgentCaseRetryRecord::from_failed_run(&scored).is_none());

    let failed = AgentCaseRunRecord::failed_attempt(FailedAgentCaseRun {
        run_id: scored.run_id,
        candidate: scored.candidate,
        case: scored.case,
        partition: scored.partition,
        attempt: retry_attempt,
        session: retry.session,
        outputs: Vec::new(),
        error: AgentCaseRunError::Scoring("not parseable".to_owned()),
        cost: Cost::llm_calls(2),
    });
    let retry_from_failure = AgentCaseRetryRecord::from_failed_run(&failed).unwrap();
    assert_eq!(retry_from_failure.attempt, retry_attempt);
    assert_eq!(
        retry_from_failure.error,
        AgentCaseRunError::Scoring("not parseable".to_owned())
    );

    let ratio = FiniteRatio::new(NonZeroUsize::new(1).unwrap(), NonZeroUsize::new(3).unwrap());
    let policy = AgentCaseRunPolicy {
        retry_on_error: 2,
        score_on_error: true,
        fail_on_error: FailOnError::Fraction(ratio),
        max_parallel_cases: Some(NonZeroUsize::new(4).unwrap()),
        max_parallel_workspaces: Some(NonZeroUsize::new(2).unwrap()),
        limits: leaven_agentic::AgentCaseLimits {
            message_limit: Some(NonZeroUsize::new(12).unwrap()),
            token_limit: Some(NonZeroUsize::new(2048).unwrap()),
            time_limit: Some(std::time::Duration::from_secs(30)),
            working_time_limit: Some(std::time::Duration::from_secs(20)),
            cost_limit: Some(Cost::llm_calls(3)),
        },
        approval: Some(ToolApprovalPolicy {
            require_approval: true,
            allowed_tools: vec!["Bash(git:*)".to_owned()],
        }),
        checkpoint: CaseCheckpointPolicy::BestEffort,
    };

    assert_eq!(ratio.numerator().get(), 1);
    assert_eq!(ratio.denominator().get(), 3);
    assert!(matches!(policy.fail_on_error, FailOnError::Fraction(_)));
    assert_eq!(policy.approval.unwrap().allowed_tools, ["Bash(git:*)"]);
}

#[test]
fn preflight_reports_cases_partitions_hidden_targets_artifact_and_runtime() {
    let hidden = ContentId::from_bytes([9; 32]);
    let suite = CaseSuite::from_cases([AgentCase::text(
        CaseId::new(1),
        "question",
        CaseTarget::Hidden(hidden),
    )])
    .unwrap();
    let workload = AgentWorkload::new(suite);
    let runtime = FakeAgentRuntime::new(Vec::new());

    let report = AgentRunPreflight::new()
        .artifact(&ValidArtifact)
        .workload(&workload)
        .runtime(&runtime)
        .check();

    assert!(!report.has_errors());
    assert!(report.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Warning
            && finding.check == "trust"
            && finding.message.contains("hidden targets")
    }));
    assert!(
        report
            .findings()
            .iter()
            .any(|finding| finding.check == "runtime")
    );
    assert!(
        report
            .findings()
            .iter()
            .any(|finding| finding.check == "artifact")
    );
}

#[test]
fn preflight_flags_empty_case_suite_and_artifact_validation_errors() {
    let suite = CaseSuite::from_cases([]).unwrap();
    let workload = AgentWorkload::new(suite);

    let report = AgentRunPreflight::new()
        .artifact(&InvalidArtifact)
        .workload(&workload)
        .check();

    assert!(report.has_errors());
    assert!(report.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Error
            && finding.check == "artifact"
            && finding.message.contains("invalid artifact")
    }));
    assert!(report.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Error && finding.check == "cases"
    }));
    assert!(report.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Error && finding.check == "partitions"
    }));
}

#[test]
fn preflight_flags_cases_without_partitions() {
    let case = AgentCase::text(CaseId::new(77), "input", CaseTarget::None);
    let mut cases = BTreeMap::new();
    cases.insert(case.id, case);
    let suite = CaseSuite::new(cases, CasePartitions::default()).unwrap();

    let report = AgentRunPreflight::new().case_suite(&suite).check();

    assert!(report.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Error
            && finding.message == "case suite has no partitions"
    }));
}

#[test]
fn preflight_checks_output_contract_shape_without_running_runtime() {
    let ok = AgentRunPreflight::new()
        .output_contract(&OutputContract::Files {
            paths: vec![WorkspacePath::new("output/result.txt").unwrap()],
        })
        .check();
    assert!(!ok.has_errors());

    let empty_files = AgentRunPreflight::new()
        .output_contract(&OutputContract::Files { paths: Vec::new() })
        .check();
    assert!(empty_files.has_errors());

    let empty_diff = AgentRunPreflight::new()
        .output_contract(&OutputContract::WorkspaceDiff {
            roots: Vec::new(),
            surface_fingerprint: None,
        })
        .check();
    assert!(empty_diff.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Warning
            && finding.check == "output-contract"
            && finding.message.contains("no explicit roots")
    }));

    let json_with_schema = AgentRunPreflight::new()
        .output_contract(&OutputContract::JsonFile {
            path: WorkspacePath::new("output/result.json").unwrap(),
            schema: Some(JsonSchemaRef {
                name: "result".to_owned(),
                schema: "{}".to_owned(),
            }),
        })
        .check();
    assert!(!json_with_schema.has_errors());

    let json_without_schema = AgentRunPreflight::new()
        .output_contract(&OutputContract::JsonFile {
            path: WorkspacePath::new("output/result.json").unwrap(),
            schema: None,
        })
        .check();
    assert!(!json_without_schema.has_errors());

    let final_message = AgentRunPreflight::new()
        .output_contract(&OutputContract::FinalMessage)
        .check();
    assert!(!final_message.has_errors());

    let structured_final_message = AgentRunPreflight::new()
        .output_contract(&OutputContract::JsonSchema {
            schema_fingerprint: "fp_schema_sha256_agentout".to_owned(),
            schema: serde_json::json!({"type": "object"}),
        })
        .check();
    assert!(!structured_final_message.has_errors());

    let diff_roots = AgentRunPreflight::new()
        .output_contract(&OutputContract::WorkspaceDiff {
            roots: vec![WorkspacePath::new("skills").unwrap()],
            surface_fingerprint: None,
        })
        .check();
    assert!(!diff_roots.has_errors());
}

#[test]
fn preflight_checks_store_capabilities_and_cache_identity_policy() {
    let store = InlineStore::new("preflight");

    let ok = AgentRunPreflight::new()
        .store(&store)
        .cache_identity(&ValidArtifact, &CachePolicy::Never)
        .cache_identity(
            &ValidArtifact,
            &CachePolicy::UserKey(Fingerprint::from_bytes([9; 32])),
        )
        .check();

    assert!(!ok.has_errors());
    assert!(ok.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Ok && finding.check == "store-blob"
    }));
    assert!(ok.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Ok && finding.check == "store-checkpoint"
    }));

    let missing_identity = AgentRunPreflight::new()
        .cache_identity(&ValidArtifact, &CachePolicy::Deterministic)
        .check();

    assert!(missing_identity.has_errors());
    assert!(missing_identity.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Error && finding.check == "cache"
    }));

    let cacheable = AgentRunPreflight::new()
        .cache_identity(&CacheableArtifact, &CachePolicy::DeterministicWithSeed(7))
        .check();
    assert!(!cacheable.has_errors());
    assert!(cacheable.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Ok
            && finding.check == "cache"
            && finding.message.contains("available")
    }));

    let failing_store = FailingStore;
    let store_errors = AgentRunPreflight::new().store(&failing_store).check();
    assert!(store_errors.has_errors());
    assert!(store_errors.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Error && finding.check == "store-blob"
    }));
    assert!(store_errors.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Error && finding.check == "store-checkpoint"
    }));
}

#[test]
fn preflight_dry_runs_presenter_without_running_runtime() {
    futures::executor::block_on(async {
        let case = AgentCase::text(CaseId::new(0), "question", CaseTarget::None);
        let artifact = CaseArtifact("seed".to_owned());
        let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
        let candidate = ctx.insert_seed(artifact.clone(), 0).unwrap();

        let report = AgentRunPreflight::new()
            .presenter_dry_run(PresenterDryRun {
                candidate_id: candidate,
                candidate: &artifact,
                case: &case,
                factory: &LocalWorkspaceFactory::temp(),
                workspace_config: WorkspaceConfig::default(),
                presenter: &TestPresenter,
                ctx: ctx.materialize_context(),
            })
            .await
            .check();

        assert!(!report.has_errors());
        assert!(
            report
                .findings()
                .iter()
                .any(|finding| finding.check == "presenter")
        );
        assert!(
            report
                .findings()
                .iter()
                .any(|finding| finding.check == "output-contract")
        );
    });
}

#[test]
fn preflight_dry_runs_scorer_with_seeded_workspace() {
    futures::executor::block_on(async {
        let case = AgentCase::text(CaseId::new(0), "question", CaseTarget::None);
        let artifact = CaseArtifact("seed".to_owned());
        let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
        let candidate = ctx.insert_seed(artifact, 0).unwrap();
        let presentation = AgentCasePresentation {
            request: AgentRunRequest::new(
                AgentInstructions::task("synthetic"),
                OutputContract::Files {
                    paths: vec![WorkspacePath::new("output/result.txt").unwrap()],
                },
            ),
            materialized_refs: Vec::new(),
        };
        let session = AgentSession::succeeded(AgentSessionId::new());

        let report = AgentRunPreflight::new()
            .scorer_dry_run(ScorerDryRun {
                candidate_id: candidate,
                case: &case,
                presentation: &presentation,
                session: &session,
                workspace_files: vec![(
                    WorkspacePath::new("output/result.txt").unwrap(),
                    b"observed".to_vec(),
                )],
                factory: &LocalWorkspaceFactory::temp(),
                workspace_config: WorkspaceConfig::default(),
                scorer: &TestScorer,
                graph: ctx.graph(),
            })
            .await
            .check();

        assert!(!report.has_errors());
        assert!(
            report
                .findings()
                .iter()
                .any(|finding| finding.check == "scorer")
        );
    });
}

#[test]
fn preflight_dry_runs_report_presenter_failure_modes() {
    futures::executor::block_on(async {
        let case = AgentCase::text(CaseId::new(0), "question", CaseTarget::None);
        let artifact = CaseArtifact("seed".to_owned());
        let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
        let candidate = ctx.insert_seed(artifact.clone(), 0).unwrap();

        let allocate = AgentRunPreflight::new()
            .presenter_dry_run(PresenterDryRun {
                candidate_id: candidate,
                candidate: &artifact,
                case: &case,
                factory: &RejectingFactory,
                workspace_config: WorkspaceConfig::default(),
                presenter: &TestPresenter,
                ctx: ctx.materialize_context(),
            })
            .await
            .check();
        assert_error_finding(&allocate, "presenter", "allocation failed");

        let stage = AgentRunPreflight::new()
            .presenter_dry_run(PresenterDryRun {
                candidate_id: candidate,
                candidate: &artifact,
                case: &case,
                factory: &LocalWorkspaceFactory::temp(),
                workspace_config: WorkspaceConfig::default(),
                presenter: &FailingPresenter,
                ctx: ctx.materialize_context(),
            })
            .await
            .check();
        assert_error_finding(&stage, "presenter", "dry run failed");

        let cleanup = AgentRunPreflight::new()
            .presenter_dry_run(PresenterDryRun {
                candidate_id: candidate,
                candidate: &artifact,
                case: &case,
                factory: &CleanupFailureFactory,
                workspace_config: WorkspaceConfig::default(),
                presenter: &TestPresenter,
                ctx: ctx.materialize_context(),
            })
            .await
            .check();
        assert_error_finding(&cleanup, "presenter-cleanup", "cleanup failed");

        let stage_and_cleanup = AgentRunPreflight::new()
            .presenter_dry_run(PresenterDryRun {
                candidate_id: candidate,
                candidate: &artifact,
                case: &case,
                factory: &CleanupFailureFactory,
                workspace_config: WorkspaceConfig::default(),
                presenter: &FailingPresenter,
                ctx: ctx.materialize_context(),
            })
            .await
            .check();
        assert_error_finding(&stage_and_cleanup, "presenter", "cleanup failed");
    });
}

#[test]
fn preflight_dry_runs_report_scorer_failure_modes() {
    futures::executor::block_on(async {
        let case = AgentCase::text(CaseId::new(0), "question", CaseTarget::None);
        let artifact = CaseArtifact("seed".to_owned());
        let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
        let candidate = ctx.insert_seed(artifact, 0).unwrap();
        let presentation = AgentCasePresentation {
            request: AgentRunRequest::new(
                AgentInstructions::task("synthetic"),
                OutputContract::Files {
                    paths: vec![WorkspacePath::new("output/result.txt").unwrap()],
                },
            ),
            materialized_refs: Vec::new(),
        };
        let session = AgentSession::succeeded(AgentSessionId::new());
        let files = || {
            vec![(
                WorkspacePath::new("output/result.txt").unwrap(),
                b"observed".to_vec(),
            )]
        };

        let allocate = AgentRunPreflight::new()
            .scorer_dry_run(ScorerDryRun {
                candidate_id: candidate,
                case: &case,
                presentation: &presentation,
                session: &session,
                workspace_files: files(),
                factory: &RejectingFactory,
                workspace_config: WorkspaceConfig::default(),
                scorer: &TestScorer,
                graph: ctx.graph(),
            })
            .await
            .check();
        assert_error_finding(&allocate, "scorer", "allocation failed");

        let stage = AgentRunPreflight::new()
            .scorer_dry_run(ScorerDryRun {
                candidate_id: candidate,
                case: &case,
                presentation: &presentation,
                session: &session,
                workspace_files: Vec::new(),
                factory: &LocalWorkspaceFactory::temp(),
                workspace_config: WorkspaceConfig::default(),
                scorer: &TestScorer,
                graph: ctx.graph(),
            })
            .await
            .check();
        assert_error_finding(&stage, "scorer", "dry run failed");

        let cleanup = AgentRunPreflight::new()
            .scorer_dry_run(ScorerDryRun {
                candidate_id: candidate,
                case: &case,
                presentation: &presentation,
                session: &session,
                workspace_files: files(),
                factory: &CleanupFailureFactory,
                workspace_config: WorkspaceConfig::default(),
                scorer: &TestScorer,
                graph: ctx.graph(),
            })
            .await
            .check();
        assert_error_finding(&cleanup, "scorer-cleanup", "cleanup failed");

        let stage_and_cleanup = AgentRunPreflight::new()
            .scorer_dry_run(ScorerDryRun {
                candidate_id: candidate,
                case: &case,
                presentation: &presentation,
                session: &session,
                workspace_files: Vec::new(),
                factory: &CleanupFailureFactory,
                workspace_config: WorkspaceConfig::default(),
                scorer: &TestScorer,
                graph: ctx.graph(),
            })
            .await
            .check();
        assert_error_finding(&stage_and_cleanup, "scorer", "cleanup failed");
    });
}

#[test]
fn agent_case_evaluator_runs_independent_per_case_sessions() {
    futures::executor::block_on(async {
        let case = AgentCase::text(
            CaseId::new(0),
            "question",
            CaseTarget::Text("expected".to_owned()),
        );
        let suite = CaseSuite::from_cases([case]).unwrap();
        let evaluator = AgentCaseEvaluator::new(
            AgentCaseEvaluatorConfig::new(
                EvaluatorId::from("agent-case/test"),
                Fingerprint::from_bytes([4; 32]),
            ),
            suite,
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/result.txt").unwrap(),
                bytes: b"observed".to_vec(),
            }])
            .with_cost(Cost::llm_calls(1)),
            TestPresenter,
            TestScorer,
        );
        let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let store = InlineEvidenceStore::<CaseEvidence>::new("case-evidence");
        let case_set = CaseSet::new(vec!["case-0"]);
        let candidate = {
            let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(CaseArtifact("seed".to_owned()), 0).unwrap()
        };
        let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_evidence_store(&store);

        let report = ctx
            .evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .unwrap();

        assert_eq!(report.assessment_ids.len(), 1);
        assert_eq!(report.cost.llm_calls, 1);
        assert_eq!(report.cost.metric_calls, 1);
        let assessment = ctx.graph().assessment(report.assessment_ids[0]).unwrap();
        assert!(matches!(
            assessment.target(),
            AssessmentTarget::Case {
                case,
                ..
            } if *case == CaseId::new(0)
        ));
        let evidence = ctx.assessment_evidence(report.assessment_ids[0]).unwrap();
        assert_eq!(evidence.output, "observed");
        let record = assessment
            .metadata()
            .get(&MetadataKey::from(CASE_RUN_RECORD_METADATA_KEY))
            .expect("case run record metadata");
        let MetadataValue::Json(record) = record else {
            panic!("case run record metadata should be JSON");
        };
        assert_eq!(record["case"], serde_json::json!(0));
        assert_eq!(record["score_recorded"], serde_json::json!(true));
        assert_eq!(record["outputs"], serde_json::json!(["output/result.txt"]));
        let inspection = AgenticRunInspection::from_graph(&ctx.graph());
        assert_eq!(inspection.case_runs.len(), 1);
        assert_eq!(inspection.case_runs[0].candidate, candidate);
        assert_eq!(inspection.case_runs[0].case, CaseId::new(0));
        assert_eq!(inspection.cache_events.len(), 1);
        assert_eq!(
            inspection.cache_events[0].cache,
            CacheStatus::Bypassed(CacheBypassReason::DisabledByPolicy)
        );
        assert_eq!(inspection.costs.case_run_records.llm_calls, 1);
        assert_eq!(inspection.costs.case_run_records.metric_calls, 1);
        assert!(inspection.warnings.is_empty());
    });
}

#[test]
fn agent_case_evaluator_exposes_cases_and_refuses_success_cost_overflow() {
    futures::executor::block_on(async {
        let case = AgentCase::text(
            CaseId::new(0),
            "question",
            CaseTarget::Text("expected".to_owned()),
        );
        let suite = CaseSuite::from_cases([case]).unwrap();
        let evaluator = AgentCaseEvaluator::new(
            AgentCaseEvaluatorConfig::new(
                EvaluatorId::from("agent-case/success-cost-overflow"),
                Fingerprint::from_bytes([4; 32]),
            ),
            suite,
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/result.txt").unwrap(),
                bytes: b"observed".to_vec(),
            }])
            .with_cost(Cost {
                metric_calls: u64::MAX,
                ..Cost::zero()
            }),
            TestPresenter,
            TestScorer,
        );
        assert_eq!(evaluator.cases().cases().len(), 1);

        let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let store = InlineEvidenceStore::<CaseEvidence>::new("case-evidence");
        let case_set = CaseSet::new(vec!["case-0"]);
        let candidate = {
            let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(CaseArtifact("seed".to_owned()), 0).unwrap()
        };
        let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_evidence_store(&store);

        let error = ctx
            .evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .unwrap_err();

        assert_case_run_error(error, "scoring");
    });
}

#[test]
fn agentic_run_inspection_reports_best_lineage_costs_and_malformed_case_metadata() {
    futures::executor::block_on(async {
        let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let store = InlineEvidenceStore::<CaseEvidence>::new("case-evidence");
        let case_set = CaseSet::new(vec!["case-0"]);
        let (seed, child) = {
            let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
            let seed = ctx.insert_seed(CaseArtifact("seed".to_owned()), 0).unwrap();
            let proposal = leaven_core::Proposal::mutate(seed, "child".to_owned()).build();
            let batch = ctx
                .record_proposal_batch(
                    leaven_kernel::StageId::custom("inspection"),
                    leaven_core::ProposalBatch {
                        proposals: vec![proposal],
                        semantics: leaven_core::ProposalBatchSemantics::Alternatives,
                        metadata: leaven_kernel::MetadataBag::new(),
                    },
                    Cost::zero(),
                )
                .unwrap()
                .batch_id;
            let child = ctx
                .apply_batch(batch)
                .unwrap()
                .successful_candidates()
                .next()
                .unwrap();
            (seed, child)
        };
        let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_evidence_store(&store);

        ctx.evaluate_with(
            &MalformedMetadataEvaluator,
            EvaluationRequest::Independent {
                candidates: vec![child],
                set: EvaluationSet::All,
                granularity: AssessmentGranularity::Aggregate,
                purpose: EvaluationPurpose::Search,
            },
        )
        .await
        .unwrap();
        ctx.emit(RunEvent::OptimizationEnded {
            run_id: ctx.graph().run_id(),
            best: Some(child),
            budget: ctx.budget(),
        });

        let inspection = AgenticRunInspection::from_graph(&ctx.graph());
        assert_eq!(inspection.best_candidate, Some(child));
        assert_eq!(inspection.best_lineage, vec![child, seed]);
        assert_eq!(inspection.costs.evaluation_events.metric_calls, 1);
        assert!(inspection.case_runs.is_empty());
        assert!(inspection.warnings.iter().any(|warning| matches!(
            warning,
            AgenticInspectionWarning::MalformedCaseRunRecordMetadata { .. }
        )));
    });
}

#[test]
fn agentic_run_inspection_warns_when_best_candidate_is_not_in_graph() {
    let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
    let missing = leaven_kernel::CandidateId::new();
    ctx.emit(RunEvent::OptimizationEnded {
        run_id: ctx.graph().run_id(),
        best: Some(missing),
        budget: ctx.budget(),
    });

    let inspection = AgenticRunInspection::from_graph(&ctx.graph());
    assert_eq!(inspection.best_candidate, Some(missing));
    assert!(inspection.best_lineage.is_empty());
    assert!(inspection.warnings.iter().any(|warning| matches!(
        warning,
        AgenticInspectionWarning::BestCandidateMissing { candidate } if *candidate == missing
    )));
}

#[test]
fn agent_case_evaluator_retries_case_errors_and_records_attempt_history() {
    futures::executor::block_on(async {
        let case = AgentCase::text(
            CaseId::new(0),
            "question",
            CaseTarget::Text("expected".to_owned()),
        );
        let suite = CaseSuite::from_cases([case]).unwrap();
        let config = AgentCaseEvaluatorConfig::new(
            EvaluatorId::from("agent-case/retry"),
            Fingerprint::from_bytes([4; 32]),
        )
        .with_run_policy(AgentCaseRunPolicy {
            retry_on_error: 1,
            ..AgentCaseRunPolicy::default()
        });
        let evaluator = AgentCaseEvaluator::new(
            config,
            suite,
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/result.txt").unwrap(),
                bytes: b"observed".to_vec(),
            }])
            .with_cost(Cost::llm_calls(1)),
            TestPresenter,
            FlakyScorer::default(),
        );
        let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let store = InlineEvidenceStore::<CaseEvidence>::new("case-evidence");
        let case_set = CaseSet::new(vec!["case-0"]);
        let candidate = {
            let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(CaseArtifact("seed".to_owned()), 0).unwrap()
        };
        let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_evidence_store(&store);

        let report = ctx
            .evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .unwrap();

        assert_eq!(report.cost.llm_calls, 2);
        assert_eq!(report.cost.metric_calls, 1);
        let assessment = ctx.graph().assessment(report.assessment_ids[0]).unwrap();
        let record = assessment
            .metadata()
            .get(&MetadataKey::from(CASE_RUN_RECORD_METADATA_KEY))
            .expect("case run record metadata");
        let MetadataValue::Json(record) = record else {
            panic!("case run record metadata should be JSON");
        };
        assert_eq!(record["attempt"], serde_json::json!(2));
        assert_eq!(record["retries"][0]["attempt"], serde_json::json!(1));
        assert_eq!(
            record["retries"][0]["error"]["kind"],
            serde_json::json!("scoring")
        );
    });
}

#[test]
fn agent_case_evaluator_refuses_retry_history_cost_overflow_before_recording_score() {
    futures::executor::block_on(async {
        let case = AgentCase::text(
            CaseId::new(0),
            "question",
            CaseTarget::Text("expected".to_owned()),
        );
        let suite = CaseSuite::from_cases([case]).unwrap();
        let config = AgentCaseEvaluatorConfig::new(
            EvaluatorId::from("agent-case/retry-cost-overflow"),
            Fingerprint::from_bytes([4; 32]),
        )
        .with_run_policy(AgentCaseRunPolicy {
            retry_on_error: 1,
            ..AgentCaseRunPolicy::default()
        });
        let evaluator = AgentCaseEvaluator::new(
            config,
            suite,
            LocalWorkspaceFactory::temp(),
            FirstAttemptMaxCostRuntime::default(),
            TestPresenter,
            FlakyScorer::default(),
        );
        let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let store = InlineEvidenceStore::<CaseEvidence>::new("case-evidence");
        let case_set = CaseSet::new(vec!["case-0"]);
        let candidate = {
            let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(CaseArtifact("seed".to_owned()), 0).unwrap()
        };
        let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_evidence_store(&store);

        let error = ctx
            .evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .unwrap_err();

        let leaven_engine::RunContextError::Evaluation(
            leaven_engine::EvaluationError::WithSource { source, .. },
        ) = error
        else {
            panic!("expected sourced evaluation error");
        };
        let source = source
            .downcast_ref::<AgenticAdapterError>()
            .expect("agentic adapter source");
        let AgenticAdapterError::CaseRunFailed {
            records_len,
            records,
            ..
        } = source
        else {
            panic!("expected case-run failure");
        };
        assert_eq!(*records_len, 2);
        assert_eq!(records.len(), 2);
        assert_eq!(
            serde_json::to_value(&records[0].error).unwrap()["kind"],
            serde_json::json!("scoring")
        );
        assert_eq!(
            serde_json::to_value(&records[1].error).unwrap()["kind"],
            serde_json::json!("scoring")
        );
        assert!(records.iter().all(|record| !record.score_recorded));
    });
}

#[test]
fn agent_case_evaluator_exhausted_retries_surface_attempt_records() {
    futures::executor::block_on(async {
        let case = AgentCase::text(
            CaseId::new(0),
            "question",
            CaseTarget::Text("expected".to_owned()),
        );
        let suite = CaseSuite::from_cases([case]).unwrap();
        let config = AgentCaseEvaluatorConfig::new(
            EvaluatorId::from("agent-case/retry-exhausted"),
            Fingerprint::from_bytes([4; 32]),
        )
        .with_run_policy(AgentCaseRunPolicy {
            retry_on_error: 1,
            ..AgentCaseRunPolicy::default()
        });
        let evaluator = AgentCaseEvaluator::new(
            config,
            suite,
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/result.txt").unwrap(),
                bytes: b"observed".to_vec(),
            }])
            .with_cost(Cost::llm_calls(1)),
            TestPresenter,
            AlwaysFailingScorer,
        );
        let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let store = InlineEvidenceStore::<CaseEvidence>::new("case-evidence");
        let case_set = CaseSet::new(vec!["case-0"]);
        let candidate = {
            let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(CaseArtifact("seed".to_owned()), 0).unwrap()
        };
        let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_evidence_store(&store);

        let error = ctx
            .evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .unwrap_err();

        let leaven_engine::RunContextError::Evaluation(
            leaven_engine::EvaluationError::WithSource { source, .. },
        ) = error
        else {
            panic!("expected sourced evaluation error");
        };
        let source = source
            .downcast_ref::<AgenticAdapterError>()
            .expect("agentic adapter source");
        let AgenticAdapterError::CaseRunFailed {
            records_len,
            records,
            ..
        } = source
        else {
            panic!("expected case-run failure");
        };
        assert_eq!(*records_len, 2);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].attempt.get(), 1);
        assert_eq!(records[1].attempt.get(), 2);
        assert!(records.iter().all(|record| !record.score_recorded));
    });
}

#[test]
fn agent_case_evaluator_records_allocate_presentation_runtime_and_cleanup_failures() {
    futures::executor::block_on(async {
        let allocate = evaluate_with_case_evaluator(
            RejectingFactory,
            FakeAgentRuntime::new(Vec::new()),
            TestPresenter,
            TestScorer,
        )
        .await
        .unwrap_err();
        assert_case_run_error(allocate, "presentation");

        let presentation = evaluate_with_case_evaluator(
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(Vec::new()),
            FailingPresenter,
            TestScorer,
        )
        .await
        .unwrap_err();
        assert_case_run_error(presentation, "presentation");

        let runtime = evaluate_with_case_evaluator(
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::ReadFile {
                path: WorkspacePath::new("missing.txt").unwrap(),
            }]),
            TestPresenter,
            TestScorer,
        )
        .await
        .unwrap_err();
        assert_case_run_error(runtime, "runtime");

        let cleanup = evaluate_with_case_evaluator(
            CleanupFailureFactory,
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/result.txt").unwrap(),
                bytes: b"observed".to_vec(),
            }]),
            TestPresenter,
            TestScorer,
        )
        .await
        .unwrap_err();
        assert_case_run_error(cleanup, "cleanup");

        let stage_and_cleanup = evaluate_with_case_evaluator(
            CleanupFailureFactory,
            FakeAgentRuntime::new(Vec::new()),
            FailingPresenter,
            TestScorer,
        )
        .await
        .unwrap_err();
        assert_case_run_error(stage_and_cleanup, "cleanup");
    });
}

#[test]
fn agent_case_evaluator_fingerprint_includes_runtime_presenter_scorer_and_cases() {
    let suite = CaseSuite::from_cases([AgentCase::text(
        CaseId::new(0),
        "question",
        CaseTarget::None,
    )])
    .unwrap();
    let config = AgentCaseEvaluatorConfig::new(
        EvaluatorId::from("agent-case/fingerprint"),
        Fingerprint::from_bytes([4; 32]),
    );
    let base = AgentCaseEvaluator::<CaseProblem, _, _, _, _>::new(
        config.clone(),
        suite.clone(),
        LocalWorkspaceFactory::temp(),
        FakeAgentRuntime::new(Vec::new()),
        TestPresenter,
        TestScorer,
    );
    let changed_scorer = AgentCaseEvaluator::<CaseProblem, _, _, _, _>::new(
        config,
        suite,
        LocalWorkspaceFactory::temp(),
        FakeAgentRuntime::new(Vec::new()),
        TestPresenter,
        OtherScorer,
    );

    assert_ne!(
        leaven_engine::Evaluator::<CaseProblem>::fingerprint(&base),
        leaven_engine::Evaluator::<CaseProblem>::fingerprint(&changed_scorer)
    );
}

async fn evaluate_with_case_evaluator<Factory, Runtime, Presenter, Scorer>(
    factory: Factory,
    runtime: Runtime,
    presenter: Presenter,
    scorer: Scorer,
) -> Result<leaven_engine::EvaluationReport, leaven_engine::RunContextError>
where
    Factory: WorkspaceFactory,
    Runtime: leaven_agent::AgentRuntime,
    Presenter: AgentCasePresenter<CaseProblem>,
    Scorer: AgentCaseScorer<CaseProblem>,
{
    let suite = CaseSuite::from_cases([AgentCase::text(
        CaseId::new(0),
        "question",
        CaseTarget::Text("expected".to_owned()),
    )])
    .unwrap();
    let evaluator = AgentCaseEvaluator::new(
        AgentCaseEvaluatorConfig::new(
            EvaluatorId::from("agent-case/failure"),
            Fingerprint::from_bytes([4; 32]),
        ),
        suite,
        factory,
        runtime,
        presenter,
        scorer,
    );
    let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let store = InlineEvidenceStore::<CaseEvidence>::new("case-evidence");
    let case_set = CaseSet::new(vec!["case-0"]);
    let candidate = {
        let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
        ctx.insert_seed(CaseArtifact("seed".to_owned()), 0).unwrap()
    };
    let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget)
        .with_case_set(&case_set)
        .with_evidence_store(&store);

    ctx.evaluate_with(
        &evaluator,
        EvaluationRequest::Independent {
            candidates: vec![candidate],
            set: EvaluationSet::All,
            granularity: AssessmentGranularity::PerCase,
            purpose: EvaluationPurpose::Search,
        },
    )
    .await
}

fn assert_case_run_error(error: leaven_engine::RunContextError, expected_kind: &str) {
    let leaven_engine::RunContextError::Evaluation(leaven_engine::EvaluationError::WithSource {
        source,
        ..
    }) = error
    else {
        panic!("expected sourced evaluation error");
    };
    let source = source
        .downcast_ref::<AgenticAdapterError>()
        .expect("agentic adapter source");
    let AgenticAdapterError::CaseRunFailed {
        records_len,
        records,
        ..
    } = source
    else {
        panic!("expected case-run failure");
    };
    assert_eq!(*records_len, 1);
    assert_eq!(records.len(), 1);
    assert_eq!(
        serde_json::to_value(&records[0].error).unwrap()["kind"],
        serde_json::json!(expected_kind)
    );
}

fn assert_error_finding(report: &leaven_agentic::AgentRunPreflightReport, check: &str, text: &str) {
    assert!(report.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Error
            && finding.check == check
            && finding.message.contains(text)
    }));
}

#[test]
fn agent_case_evaluator_rejects_unsupported_request_shapes_and_missing_inputs() {
    futures::executor::block_on(async {
        let suite = CaseSuite::from_cases([AgentCase::text(
            CaseId::new(0),
            "question",
            CaseTarget::None,
        )])
        .unwrap();
        let evaluator = AgentCaseEvaluator::new(
            AgentCaseEvaluatorConfig::new(
                EvaluatorId::from("agent-case/rejections"),
                Fingerprint::from_bytes([4; 32]),
            ),
            suite,
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(Vec::new()),
            TestPresenter,
            TestScorer,
        );
        let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let case_set = CaseSet::new(vec!["case-0", "case-1"]);
        let candidate = {
            let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(CaseArtifact("seed".to_owned()), 0).unwrap()
        };
        let mut ctx =
            RunContext::<CaseProblem>::new(&mut graph, &mut budget).with_case_set(&case_set);

        let aggregate = ctx
            .evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .unwrap_err();
        assert!(
            aggregate
                .to_string()
                .contains("agent case evaluator failed")
        );

        let pairwise = ctx
            .evaluate_with(
                &evaluator,
                EvaluationRequest::Pairwise {
                    left: candidate,
                    right: candidate,
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Search,
                    order: PairOrder::Unordered,
                },
            )
            .await
            .unwrap_err();
        assert!(pairwise.to_string().contains("agent case evaluator failed"));

        let unknown_candidate = ctx
            .evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![leaven_kernel::CandidateId::new()],
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .unwrap_err();
        assert!(
            unknown_candidate
                .to_string()
                .contains("agent case evaluator failed")
        );

        let missing_case = ctx
            .evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .unwrap_err();
        assert!(
            missing_case
                .to_string()
                .contains("agent case evaluator failed")
        );
    });
}

#[test]
fn agent_case_evaluator_rejects_resolved_cases_missing_from_suite() {
    futures::executor::block_on(async {
        let suite = CaseSuite::from_cases([AgentCase::text(
            CaseId::new(0),
            "question",
            CaseTarget::Text("expected".to_owned()),
        )])
        .unwrap();
        let evaluator = AgentCaseEvaluator::new(
            AgentCaseEvaluatorConfig::new(
                EvaluatorId::from("agent-case/direct-missing-case"),
                Fingerprint::from_bytes([4; 32]),
            ),
            suite,
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/result.txt").unwrap(),
                bytes: b"observed".to_vec(),
            }]),
            TestPresenter,
            TestScorer,
        );
        let mut graph = RunGraph::<CaseProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let engine_cases = CaseSet::new(vec!["case-0", "case-1"]);
        let candidate = {
            let mut ctx = RunContext::<CaseProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(CaseArtifact("seed".to_owned()), 0).unwrap()
        };
        let mut ctx =
            RunContext::<CaseProblem>::new(&mut graph, &mut budget).with_case_set(&engine_cases);
        let resolved = engine_cases.resolve(&EvaluationSet::All).unwrap();

        let result = evaluator
            .evaluate(
                ResolvedEvaluationRequest {
                    kind: leaven_core::ResolvedRequestKind::Independent {
                        candidates: vec![candidate],
                    },
                    set: resolved,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Search,
                },
                ctx.evaluation_context(StageId::custom("agent-case-test")),
            )
            .await;
        let Err(error) = result else {
            panic!("case suite mismatch should fail evaluation");
        };

        assert!(format!("{error:?}").contains("case suite does not contain resolved case"));
    });
}

#[derive(Clone)]
struct ValidArtifact;

impl Artifact for ValidArtifact {
    type Change = ();
    type ApplyError = TestArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::External("valid".to_owned())
    }

    fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self)
    }
}

#[derive(Clone)]
struct CacheableArtifact;

impl Artifact for CacheableArtifact {
    type Change = ();
    type ApplyError = TestArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::External("cacheable".to_owned())
    }

    fn cache_identity(&self) -> Option<CacheIdentity> {
        Some(CacheIdentity::Content(ContentId::from_bytes([3; 32])))
    }

    fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self)
    }
}

#[derive(Clone)]
struct InvalidArtifact;

impl Artifact for InvalidArtifact {
    type Change = ();
    type ApplyError = TestArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::External("invalid".to_owned())
    }

    fn validate(&self) -> Result<(), Self::ApplyError> {
        Err(TestArtifactError)
    }

    fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid artifact")]
struct TestArtifactError;

#[derive(Clone, Debug, Eq, PartialEq)]
struct CaseArtifact(String);

impl Artifact for CaseArtifact {
    type Change = String;
    type ApplyError = TestArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::External(self.0.clone())
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(change.clone()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CaseEvidence {
    output: String,
}

impl leaven_core::Evidence for CaseEvidence {}

struct CaseProblem;

impl OptimizationProblem for CaseProblem {
    type Artifact = CaseArtifact;
    type Case = &'static str;
    type Evidence = CaseEvidence;
    type ProposalAnnotations = ();
}

struct TestPresenter;

impl AgentCasePresenter<CaseProblem> for TestPresenter {
    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([5; 32])
    }

    async fn present(
        &self,
        input: AgentCasePresentationInput<'_, CaseProblem>,
        workspace: &mut WorkspaceView<'_>,
        _ctx: MaterializeContext<'_, CaseProblem>,
    ) -> Result<Metered<AgentCasePresentation>, AgenticAdapterError> {
        workspace.write_file(
            &WorkspacePath::new("candidate.txt").unwrap(),
            input.candidate.0.as_bytes(),
        )?;
        workspace.write_file(
            &WorkspacePath::new("case.txt").unwrap(),
            format!("{:?}", input.case.input).as_bytes(),
        )?;
        Ok(Metered::new(
            AgentCasePresentation {
                request: AgentRunRequest::new(
                    AgentInstructions::task("write output"),
                    OutputContract::Files {
                        paths: vec![WorkspacePath::new("output/result.txt").unwrap()],
                    },
                ),
                materialized_refs: vec![
                    WorkspacePath::new("candidate.txt").unwrap(),
                    WorkspacePath::new("case.txt").unwrap(),
                ],
            },
            Cost::zero(),
        ))
    }
}

struct FailingPresenter;

impl AgentCasePresenter<CaseProblem> for FailingPresenter {
    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([10; 32])
    }

    async fn present(
        &self,
        _input: AgentCasePresentationInput<'_, CaseProblem>,
        _workspace: &mut WorkspaceView<'_>,
        _ctx: MaterializeContext<'_, CaseProblem>,
    ) -> Result<Metered<AgentCasePresentation>, AgenticAdapterError> {
        Err(AgenticAdapterError::Input("presenter failed".to_owned()))
    }
}

struct TestScorer;

impl AgentCaseScorer<CaseProblem> for TestScorer {
    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([6; 32])
    }

    async fn score(
        &self,
        _input: AgentCaseScoreInput<'_, CaseProblem>,
        workspace: &WorkspaceView<'_>,
    ) -> Result<Metered<CaseEvidence>, AgenticAdapterError> {
        let output = workspace.read_file(&WorkspacePath::new("output/result.txt").unwrap())?;
        Ok(Metered::new(
            CaseEvidence {
                output: String::from_utf8(output).unwrap(),
            },
            Cost::metric_calls(1),
        ))
    }
}

struct MalformedMetadataEvaluator;

impl Evaluator<CaseProblem> for MalformedMetadataEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::from("malformed-metadata")
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
        _ctx: EvaluationContext<'_, CaseProblem>,
    ) -> Result<Metered<Vec<Assessment<CaseProblem>>>, EvaluationError> {
        let leaven_core::ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message("expected independent".to_owned()));
        };
        let mut metadata = leaven_kernel::MetadataBag::new();
        metadata.insert(
            CASE_RUN_RECORD_METADATA_KEY,
            MetadataValue::String("not json".to_owned()),
        );
        Ok(Metered::new(
            vec![Assessment::Independent {
                candidate: candidates[0],
                target: AssessmentTarget::Unscoped,
                evidence: CaseEvidence {
                    output: "observed".to_owned(),
                },
                cost: Cost::metric_calls(1),
                metadata,
            }],
            Cost::metric_calls(1),
        ))
    }
}

#[derive(Clone, Default)]
struct FlakyScorer {
    attempts: Arc<AtomicUsize>,
}

impl AgentCaseScorer<CaseProblem> for FlakyScorer {
    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([8; 32])
    }

    async fn score(
        &self,
        input: AgentCaseScoreInput<'_, CaseProblem>,
        workspace: &WorkspaceView<'_>,
    ) -> Result<Metered<CaseEvidence>, AgenticAdapterError> {
        if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(AgenticAdapterError::Input(
                "synthetic scorer failure".to_owned(),
            ));
        }
        TestScorer.score(input, workspace).await
    }
}

struct AlwaysFailingScorer;

impl AgentCaseScorer<CaseProblem> for AlwaysFailingScorer {
    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([9; 32])
    }

    async fn score(
        &self,
        _input: AgentCaseScoreInput<'_, CaseProblem>,
        _workspace: &WorkspaceView<'_>,
    ) -> Result<Metered<CaseEvidence>, AgenticAdapterError> {
        Err(AgenticAdapterError::Input(
            "synthetic permanent scorer failure".to_owned(),
        ))
    }
}

#[derive(Clone, Default)]
struct FirstAttemptMaxCostRuntime {
    attempts: Arc<AtomicUsize>,
}

impl AgentRuntime for FirstAttemptMaxCostRuntime {
    fn id(&self) -> leaven_kernel::AgentRuntimeId {
        leaven_kernel::AgentRuntimeId::new_const("first-attempt-max-cost")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([13; 32])
    }

    async fn run_session(
        &self,
        workspace: &mut WorkspaceView<'_>,
        _request: AgentRunRequest,
        ctx: AgentRunContext<'_>,
    ) -> Result<Metered<AgentSession>, AgentRuntimeError> {
        workspace.write_file(
            &WorkspacePath::new("output/result.txt").unwrap(),
            b"observed",
        )?;
        let mut session = AgentSession::succeeded(ctx.session_id());
        session
            .output_files
            .push(WorkspacePath::new("output/result.txt").unwrap());
        let cost = if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            Cost {
                metric_calls: u64::MAX,
                ..Cost::zero()
            }
        } else {
            Cost::zero()
        };
        Ok(Metered::new(session, cost))
    }
}

struct OtherScorer;

impl AgentCaseScorer<CaseProblem> for OtherScorer {
    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([7; 32])
    }

    async fn score(
        &self,
        input: AgentCaseScoreInput<'_, CaseProblem>,
        workspace: &WorkspaceView<'_>,
    ) -> Result<Metered<CaseEvidence>, AgenticAdapterError> {
        TestScorer.score(input, workspace).await
    }
}

struct RejectingFactory;

impl WorkspaceFactory for RejectingFactory {
    async fn allocate(&self, _config: WorkspaceConfig) -> Result<Workspace, FactoryError> {
        Err(FactoryError::Allocate("no workspace".to_owned()))
    }
}

struct CleanupFailureFactory;

impl WorkspaceFactory for CleanupFailureFactory {
    async fn allocate(&self, _config: WorkspaceConfig) -> Result<Workspace, FactoryError> {
        Ok(Workspace::new(
            PathBuf::new(),
            Box::new(CleanupFailureBackend {
                files: BTreeMap::new(),
            }),
        ))
    }
}

struct CleanupFailureBackend {
    files: BTreeMap<WorkspacePath, Vec<u8>>,
}

impl WorkspaceBackend for CleanupFailureBackend {
    fn write_file(&mut self, path: &WorkspacePath, bytes: &[u8]) -> Result<(), WorkspaceError> {
        self.files.insert(path.clone(), bytes.to_vec());
        Ok(())
    }

    fn read_file(&mut self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| WorkspaceError::Io(format!("missing {}", path.as_str())))
    }

    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>> {
        async { Err(WorkspaceError::Cleanup("cleanup failed".to_owned())) }.boxed()
    }
}

struct FailingStore;

impl BlobStore for FailingStore {
    fn put(&self, _write: BlobWrite) -> Result<BlobRef, StoreError> {
        Err(StoreError::OperationFailed {
            store: "failing".to_owned(),
            operation: "put_blob",
            reason: "blob offline".to_owned(),
            retryable: Some(true),
        })
    }

    fn get(&self, reference: &BlobRef) -> Result<bytes::Bytes, StoreError> {
        Err(StoreError::BlobNotFound(reference.clone()))
    }
}

impl CheckpointStore for FailingStore {
    fn put(&self, _checkpoint: CheckpointBytes) -> Result<CheckpointId, StoreError> {
        Err(StoreError::OperationFailed {
            store: "failing".to_owned(),
            operation: "put_checkpoint",
            reason: "checkpoint offline".to_owned(),
            retryable: Some(true),
        })
    }

    fn get(&self, id: CheckpointId) -> Result<CheckpointBytes, StoreError> {
        Err(StoreError::OperationFailed {
            store: "failing".to_owned(),
            operation: "get_checkpoint",
            reason: format!("missing {id}"),
            retryable: Some(false),
        })
    }

    fn latest(&self) -> Result<Option<CheckpointId>, StoreError> {
        Ok(None)
    }

    fn mark_latest(&self, _id: CheckpointId) -> Result<(), StoreError> {
        Err(StoreError::OperationFailed {
            store: "failing".to_owned(),
            operation: "mark_latest_checkpoint",
            reason: "checkpoint offline".to_owned(),
            retryable: Some(true),
        })
    }
}
