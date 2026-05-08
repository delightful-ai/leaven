use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use leaven_agent::{
    AgentInstructions, AgentRunRequest, AgentSession, FakeAgentAction, FakeAgentRuntime,
    JsonSchemaRef, OutputContract,
};
use leaven_agentic::{
    AgentCase, AgentCaseEvaluator, AgentCaseEvaluatorConfig, AgentCasePresentation,
    AgentCasePresentationInput, AgentCasePresenter, AgentCaseRunPolicy, AgentCaseScoreInput,
    AgentCaseScorer, AgentRunPreflight, AgentWorkload, AgenticAdapterError, AgenticRunInspection,
    CASE_RUN_RECORD_METADATA_KEY, CasePartitionId, CasePartitions, CaseSuite, CaseTarget,
    PreflightSeverity, PresenterDryRun, ScorerDryRun,
};
use leaven_core::{
    Artifact, ArtifactIdentity, AssessmentGranularity, AssessmentTarget, CacheIdentity,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, OptimizationProblem, PairOrder,
};
use leaven_engine::{
    BudgetLedger, CacheBypassReason, CachePolicy, CacheStatus, CaseSet, MaterializeContext,
    RunContext, RunGraph,
};
use leaven_kernel::{
    AgentSessionId, CaseId, ContentId, Cost, EvaluatorId, Fingerprint, MetadataKey, MetadataValue,
    Metered, RunId,
};
use leaven_store_inline::{InlineEvidenceStore, InlineStore};
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
        .output_contract(&OutputContract::WorkspaceDiff { roots: Vec::new() })
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

    let diff_roots = AgentRunPreflight::new()
        .output_contract(&OutputContract::WorkspaceDiff {
            roots: vec![WorkspacePath::new("skills").unwrap()],
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
