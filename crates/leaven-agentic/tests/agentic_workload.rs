use std::collections::BTreeMap;

use leaven_agent::{
    AgentInstructions, AgentRunRequest, AgentSession, FakeAgentAction, FakeAgentRuntime,
    OutputContract,
};
use leaven_agentic::{
    AgentCase, AgentCaseEvaluator, AgentCaseEvaluatorConfig, AgentCasePresentation,
    AgentCasePresentationInput, AgentCasePresenter, AgentCaseScoreInput, AgentCaseScorer,
    AgentRunPreflight, AgentWorkload, AgenticAdapterError, CasePartitionId, CasePartitions,
    CaseSuite, CaseTarget, PreflightSeverity,
};
use leaven_core::{
    Artifact, ArtifactIdentity, AssessmentGranularity, AssessmentTarget, EvaluationPurpose,
    EvaluationRequest, EvaluationSet, OptimizationProblem,
};
use leaven_engine::{BudgetLedger, CaseSet, MaterializeContext, RunContext, RunGraph};
use leaven_kernel::{
    AgentSessionId, CaseId, ContentId, Cost, EvaluatorId, Fingerprint, Metered, RunId,
};
use leaven_store_inline::InlineEvidenceStore;
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
            .presenter_dry_run(
                candidate,
                &artifact,
                &case,
                &LocalWorkspaceFactory::temp(),
                WorkspaceConfig::default(),
                &TestPresenter,
                ctx.materialize_context(),
            )
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
            .scorer_dry_run(
                candidate,
                &case,
                &presentation,
                &session,
                [(
                    WorkspacePath::new("output/result.txt").unwrap(),
                    b"observed".to_vec(),
                )],
                &LocalWorkspaceFactory::temp(),
                WorkspaceConfig::default(),
                &TestScorer,
                ctx.graph(),
            )
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
