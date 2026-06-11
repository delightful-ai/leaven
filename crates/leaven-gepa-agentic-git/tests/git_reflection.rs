use std::collections::BTreeMap;
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use leaven_agent::{
    AgentContextRef, AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime,
    AgentSession, OutputContract,
    test_support::{FakeAgentAction, FakeAgentRuntime},
};
use leaven_agentic_git::{GitProgramMaterializer, GitProgramReadback, GitProgramStores};
use leaven_artifact_git::{
    GitArtifactIdentityMode, GitObjectId, GitPath, GitProgramArtifact, GitRepoArtifact,
    GitRevision, RepoKey, RepoRef,
};
use leaven_core::{Evidence, InfoRef, OptimizationProblem, ProposalBatch, ProposalBatchSemantics};
use leaven_engine::{
    Arity, BudgetLedger, ProposalContext, ProposalError, Proposer, RunContext, RunEvent, RunGraph,
    RunGraphView,
};
use leaven_evidence::ScalarEvidence;
use leaven_gepa::{ReflectRequest, ReflectiveCase, ReflectiveSideInfoValue, ReflectiveValue};
use leaven_gepa_agentic_git::{
    GepaGitProgramReflectionRenderer, GitProgramGepaReflectionInput,
    GitProgramGepaReflectionMaterializer, GitProgramGepaReflectionParser,
    GitProgramPublicSeamReflectionResult, GitProgramPublicSeamStageContext,
};
use leaven_kernel::{
    AgentSessionId, AssessmentId, Budget, BudgetSnapshot, CandidateId, Cost, Metered, ProposerId,
    RunId,
};
use leaven_population::{TopKFrontier, TopKParentSelector};
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory, WorkspacePath, WorkspaceView};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn git_program_gepa_reflector_materializes_agent_edit_and_applies_child() {
    let fixture = GitFixture::new();
    let seed = fixture.program_artifact();
    let mut graph = RunGraph::<GitProblem>::new(RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let parent = {
        let mut ctx = RunContext::<GitProblem>::new(&mut graph, &mut budget);
        ctx.insert_seed(seed.clone(), 0).unwrap()
    };
    let mut ctx = RunContext::<GitProblem>::new(&mut graph, &mut budget);

    let mut frontier = TopKFrontier::new(NonZeroUsize::new(1).unwrap());
    let selected_parent = initialize_tiny_frontier(&mut ctx, &mut frontier, parent);
    let input = reflection_input(seed, selected_parent);
    let proposal_batch = materialize_agent_edit_and_parse(&fixture, &input, &ctx.graph());
    assert_eq!(
        proposal_batch.semantics,
        ProposalBatchSemantics::Alternatives
    );

    let child = record_and_apply_child(&mut ctx, proposal_batch);
    let child_revision = child_revision(&ctx, child);
    assert_child_revision(&fixture, &child_revision);
    assert_eq!(ctx.graph().parents(child), vec![parent]);
    assert_provenance(&ctx, child, selected_parent);

    let report = admit_child_to_tiny_frontier(
        &mut ctx,
        &mut frontier,
        selected_parent,
        child,
        child_revision,
    );
    assert_eq!(report.parent, parent);
    assert_eq!(report.child, child);
    assert!(report.child_admitted);
    assert_eq!(report.best_candidate, Some(child));
    assert_eq!(report.best_score, Some(0.9));
    assert_ne!(report.child_revision, fixture.program_parent);
    assert!(
        ctx.graph()
            .events()
            .filter(|event| matches!(event, RunEvent::PopulationUpdated { .. }))
            .count()
            >= 2,
        "the product proof must record frontier/admission state in the run event stream"
    );
}

#[test]
fn renderer_covers_empty_and_nested_reflective_examples() {
    let fixture = GitFixture::new();
    let renderer = GepaGitProgramReflectionRenderer;
    let parent = CandidateId::new();
    let empty = empty_reflection_input(fixture.program_artifact(), parent);

    let empty_instructions = renderer.render_input(&empty).unwrap();
    assert!(
        empty_instructions
            .task
            .contains("(no reflective examples selected)")
    );

    let nested_request = ReflectRequest::for_part(
        parent,
        "repos/program/program.txt".to_owned(),
        "program.txt",
    )
    .with_examples([{
        let mut example = ReflectiveCase::from_example(
            ReflectiveValue::Text("program base".to_owned()),
            None,
            None,
            None,
            "Use the trace.",
        );
        example.runs[0].side_info = vec![(
            "Trace".to_owned(),
            ReflectiveSideInfoValue::Mapping(vec![(
                "Steps".to_owned(),
                ReflectiveSideInfoValue::List(vec![ReflectiveSideInfoValue::Text(
                    "read program.txt".to_owned(),
                )]),
            )]),
        )];
        example
    }]);
    let nested =
        GitProgramGepaReflectionInput::from_request(fixture.program_artifact(), nested_request);

    let nested_instructions = renderer.render_input(&nested).unwrap();
    assert!(nested_instructions.task.contains("##### Steps"));
    assert!(nested_instructions.task.contains("###### Item 1"));
    assert!(nested_instructions.task.contains("read program.txt"));
}

#[test]
fn path_surface_exposes_each_repo_as_a_selectable_part() {
    use leaven_surface::EditSurface;
    let fixture = GitFixture::new();
    let artifact = fixture.program_artifact();
    let surface = leaven_gepa_agentic_git::GitProgramPathSurface;

    // The single program repo is exposed as one part keyed by its repo key with
    // its materialization layout as the address.
    let parts = surface.parts(&artifact).unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].id, repo_key("program"));
    assert_eq!(parts[0].address, git_path("repos/program"));
    assert_eq!(parts[0].view, "repos/program");

    // A typed change targeting the selected repo lowers through change_part; one
    // targeting a different/absent repo is refused.
    let parent_revision = artifact
        .repo(&repo_key("program"))
        .unwrap()
        .revision()
        .clone();
    let change = leaven_artifact_git::GitProgramChange::AdvanceRepo {
        repo: repo_key("program"),
        expected_parent: parent_revision.clone(),
        child: parent_revision,
    };
    assert!(
        surface
            .change_part(&artifact, repo_key("program"), change.clone())
            .is_ok()
    );
    let bench_revision = fixture.program_parent;
    let mismatched = leaven_artifact_git::GitProgramChange::AdvanceRepo {
        repo: repo_key("bench"),
        expected_parent: bench_revision.clone(),
        child: bench_revision,
    };
    assert!(
        surface
            .change_part(&artifact, repo_key("program"), mismatched)
            .is_err(),
        "a change that does not target the selected repo must be refused"
    );
    assert!(
        surface
            .change_part(&artifact, repo_key("bench"), change)
            .is_err(),
        "selecting an absent repo part must be refused"
    );
}

#[test]
fn parser_reports_missing_parent_and_clean_workspace() {
    let fixture = GitFixture::new();
    let parser = GitProgramGepaReflectionParser::new(GitProgramReadback::new(fixture.stores()));
    let missing_parent = CandidateId::new();
    let missing_input = reflection_input(fixture.program_artifact(), missing_parent);
    let mut missing_graph = RunGraph::<GitProblem>::new(RunId::new());
    let mut missing_budget = BudgetLedger::new(Budget::unlimited());
    let missing_ctx = RunContext::<GitProblem>::new(&mut missing_graph, &mut missing_budget);
    let mut missing_workspace = futures::executor::block_on(
        LocalWorkspaceFactory::temp().allocate(WorkspaceConfig::default()),
    )
    .unwrap();
    {
        let mut view = missing_workspace.view();
        let Err(error) = parser.parse_workspace(&mut view, &missing_input, &missing_ctx.graph())
        else {
            panic!("missing parent parse should fail");
        };
        assert!(format!("{error:?}").contains("parent Git program not found"));
    }
    futures::executor::block_on(missing_workspace.cleanup()).unwrap();

    let seed = fixture.program_artifact();
    let mut graph = RunGraph::<GitProblem>::new(RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let parent = {
        let mut ctx = RunContext::<GitProblem>::new(&mut graph, &mut budget);
        ctx.insert_seed(seed.clone(), 0).unwrap()
    };
    let ctx = RunContext::<GitProblem>::new(&mut graph, &mut budget);
    let input = reflection_input(seed, parent);
    let materializer =
        GitProgramGepaReflectionMaterializer::new(GitProgramMaterializer::new(fixture.stores()));
    let mut clean_workspace = futures::executor::block_on(
        LocalWorkspaceFactory::temp().allocate(WorkspaceConfig::default()),
    )
    .unwrap();
    {
        let mut view = clean_workspace.view();
        materializer.materialize_input(&input, &mut view).unwrap();
        let Err(error) = parser.parse_workspace(&mut view, &input, &ctx.graph()) else {
            panic!("clean workspace parse should fail");
        };
        assert!(format!("{error:?}").contains("Git program workspace had no changes"));
    }
    futures::executor::block_on(clean_workspace.cleanup()).unwrap();
}

#[test]
fn materializer_reports_blocked_reflection_brief_directory() {
    let fixture = GitFixture::new();
    let materializer =
        GitProgramGepaReflectionMaterializer::new(GitProgramMaterializer::new(fixture.stores()));
    let input = empty_reflection_input(fixture.program_artifact(), CandidateId::new());
    let mut workspace = futures::executor::block_on(
        LocalWorkspaceFactory::temp().allocate(WorkspaceConfig::default()),
    )
    .unwrap();
    {
        let mut view = workspace.view();
        view.write_file(&workspace_path(".leaven"), b"not a directory\n")
            .unwrap();
        let error = materializer
            .materialize_input(&input, &mut view)
            .unwrap_err();
        assert!(format!("{error:?}").contains("failed to create GEPA reflection directory"));
    }
    futures::executor::block_on(workspace.cleanup()).unwrap();
}

#[cfg(coverage)]
#[test]
fn reflector_wrapper_runs_agentic_proposer_under_llvm_coverage() {
    futures::executor::block_on(async {
        let fixture = GitFixture::new();
        let seed = fixture.program_artifact();
        let run_id = RunId::new();
        let mut graph = RunGraph::<GitProblem>::new(run_id);
        let mut budget = BudgetLedger::new(Budget::unlimited());
        let parent = {
            let mut ctx = RunContext::<GitProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(seed, 0).unwrap()
        };
        let mut reflector = leaven_gepa_agentic_git::GepaGitProgramAgenticReflector::new(
            leaven_agentic::AgenticProposerConfig::new(ProposerId::from("gepa/git-agentic")),
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![
                FakeAgentAction::ReadFile {
                    path: workspace_path("repos/program/program.txt"),
                },
                FakeAgentAction::WriteFile {
                    path: workspace_path("repos/program/program.txt"),
                    bytes: b"program reflected through runtime\n".to_vec(),
                },
            ]),
            GitProgramMaterializer::new(fixture.stores()),
            GitProgramReadback::new(fixture.stores()),
        );
        let request = ReflectRequest::for_part(parent, repo_key("program"), "program")
            .with_examples([ReflectiveCase::from_example(
                ReflectiveValue::Text("runtime read the parent".to_owned()),
                None,
                None,
                None,
                "write the reflected body",
            )])
            .with_source_refs([InfoRef::Candidate(parent)]);
        let mut ctx = RunContext::<GitProblem>::new(&mut graph, &mut budget);

        let child = leaven_gepa::GepaReflector::reflect_candidate(
            &mut reflector,
            &mut ctx,
            &leaven_gepa_agentic_git::GitProgramPathSurface,
            request,
        )
        .await
        .unwrap()
        .unwrap();

        let revision = child_revision(&ctx, child);
        assert_eq!(
            git_output(
                &fixture.program_store,
                ["show", &format!("{}:program.txt", revision.object_id())],
            ),
            "program reflected through runtime\n"
        );
    });
}

#[test]
fn public_seam_reflect_then_propose_stage_projection_uses_separate_agent_sessions() {
    futures::executor::block_on(async {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
            .expect("public seam package loads");
        let fixture = GitFixture::new();
        let seed = fixture.program_artifact();
        let run_id = RunId::new();
        let mut graph = RunGraph::<GitProblem>::new(run_id);
        let mut budget = BudgetLedger::new(Budget::unlimited());
        let parent = {
            let mut ctx = RunContext::<GitProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(seed.clone(), 0).unwrap()
        };
        let input = reflection_input(seed, parent);
        let context = public_stage_context(run_id);
        let reflection_value = public_stage_reflection(parent);
        let proposal_batch = run_separated_public_stage_workspace(
            &package,
            &fixture,
            &input,
            &context,
            reflection_value,
            &mut graph,
            &mut budget,
        )
        .await;

        let mut ctx = RunContext::<GitProblem>::new(&mut graph, &mut budget);
        let proposer = PrebuiltProposalProposer::new(proposal_batch);
        let batch = ctx.propose(&proposer, ()).await.unwrap();
        let child = ctx
            .apply_batch(batch.batch_id)
            .unwrap()
            .successful_candidates()
            .next()
            .unwrap();
        let child_revision = child_revision(&ctx, child);
        assert_eq!(
            git_output(
                &fixture.program_store,
                [
                    "show",
                    &format!("{}:program.txt", child_revision.object_id())
                ],
            ),
            "program reflected through separated public seam stages\n"
        );
        assert_provenance(&ctx, child, parent);
    });
}

fn public_stage_context(run_id: RunId) -> GitProgramPublicSeamStageContext {
    GitProgramPublicSeamStageContext::new(
        run_id,
        "rev_gepa_git_stage_base",
        "fp_surface_sha256_gepa_git_stage",
        "fp_policy_sha256_gepa_git_stage",
        "fp_cap_sha256_gepa_git_stage",
        "fp_schema_sha256_gepa_git_stage_patch",
    )
    .with_stage_call_ids("sc_gepa_git_reflect_test", "sc_gepa_git_propose_test")
    .with_reflection_read_receipt("qrec_gepa_git_reflection_sources")
}

fn public_stage_reflection(parent: CandidateId) -> serde_json::Value {
    GitProgramPublicSeamReflectionResult::source_backed(
        "The selected Git program file should be edited.",
        vec![prefixed_candidate_ref(parent)],
        ["qrec_gepa_git_reflection_sources".to_owned()],
        "fp_surface_sha256_gepa_git_stage",
        "program.txt",
    )
    .into_value()
}

async fn run_separated_public_stage_workspace(
    package: &leaven_public_seam::PublicSeamPackage,
    fixture: &GitFixture,
    input: &GitProgramGepaReflectionInput<String>,
    context: &GitProgramPublicSeamStageContext,
    reflection_value: serde_json::Value,
    graph: &mut RunGraph<GitProblem>,
    budget: &mut BudgetLedger,
) -> ProposalBatch<GitProblem> {
    let reflection_path = workspace_path(".leaven/reflection-result.json");
    let materializer =
        GitProgramGepaReflectionMaterializer::new(GitProgramMaterializer::new(fixture.stores()));
    let parser = GitProgramGepaReflectionParser::new(GitProgramReadback::new(fixture.stores()));
    let mut workspace = LocalWorkspaceFactory::temp()
        .allocate(WorkspaceConfig::default())
        .await
        .unwrap();
    let proposal_batch = {
        let mut view = workspace.view();
        materializer.materialize_input(input, &mut view).unwrap();
        package
            .validate_stage_payload_document(&context.reflect_request(input))
            .expect("producer-built reflect request validates");

        let budget_snapshot = RunContext::<GitProblem>::new(graph, budget).budget();
        let reflect_session = run_reflector_stage(
            &mut view,
            &reflection_path,
            &reflection_value,
            &budget_snapshot,
        )
        .await;
        let runtime_reflection: serde_json::Value =
            serde_json::from_slice(&view.read_file(&reflection_path).unwrap()).unwrap();
        package
            .validate_stage_payload_document(&runtime_reflection)
            .expect("runtime-produced reflection result validates");
        package
            .validate_stage_payload_document(&context.propose_request(input, &runtime_reflection))
            .expect("producer-built propose request validates");

        let propose_session =
            run_proposer_stage(&mut view, &reflection_path, &budget_snapshot).await;
        assert_ne!(
            reflect_session.value.session_id,
            propose_session.value.session_id
        );

        let parsed = {
            let ctx_for_graph = RunContext::<GitProblem>::new(graph, budget);
            parser
                .parse_workspace(&mut view, input, &ctx_for_graph.graph())
                .unwrap()
                .value
        };
        let projection = context.project(input, &runtime_reflection, &parsed);
        let handoff = package
            .validate_reflect_propose_handoff_document(&projection.handoff)
            .expect("separate reflect/propose handoff validates");
        assert_eq!(handoff.reflect_stage_call_id(), "sc_gepa_git_reflect_test");
        assert_eq!(handoff.propose_stage_call_id(), "sc_gepa_git_propose_test");
        package
            .validate_reflect_propose_submission_document(
                &projection.handoff,
                &projection.submission_plan,
            )
            .expect("proposal submission cites stage handoff");
        parsed
    };
    workspace.cleanup().await.unwrap();
    proposal_batch
}

async fn run_reflector_stage(
    view: &mut WorkspaceView<'_>,
    reflection_path: &WorkspacePath,
    reflection_value: &serde_json::Value,
    budget: &BudgetSnapshot,
) -> Metered<AgentSession> {
    let reflect_runtime = FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
        path: reflection_path.clone(),
        bytes: serde_json::to_vec_pretty(reflection_value).unwrap(),
    }])
    .with_id("fake/reflector".into());
    reflect_runtime
        .run_session(
            view,
            AgentRunRequest::new(
                AgentInstructions::task("write the reflection result only"),
                OutputContract::JsonFile {
                    path: reflection_path.clone(),
                    schema: None,
                },
            ),
            AgentRunContext::new(AgentSessionId::new(), budget),
        )
        .await
        .unwrap()
}

async fn run_proposer_stage(
    view: &mut WorkspaceView<'_>,
    reflection_path: &WorkspacePath,
    budget: &BudgetSnapshot,
) -> Metered<AgentSession> {
    let propose_runtime = FakeAgentRuntime::new(vec![
        FakeAgentAction::ReadFile {
            path: reflection_path.clone(),
        },
        FakeAgentAction::WriteFile {
            path: workspace_path("repos/program/program.txt"),
            bytes: b"program reflected through separated public seam stages\n".to_vec(),
        },
    ])
    .with_id("fake/proposer".into());
    propose_runtime
        .run_session(
            view,
            AgentRunRequest::new(
                AgentInstructions::task("consume reflection result and edit the repo"),
                OutputContract::WorkspaceDiff {
                    roots: vec![WorkspacePath::root()],
                    surface_fingerprint: Some("fp_surface_sha256_gepa_git_stage".to_owned()),
                },
            ),
            AgentRunContext::new(AgentSessionId::new(), budget),
        )
        .await
        .unwrap()
}

fn initialize_tiny_frontier(
    ctx: &mut RunContext<'_, GitProblem>,
    frontier: &mut TopKFrontier,
    parent: CandidateId,
) -> CandidateId {
    let parent_events = frontier.observe(
        parent,
        AssessmentId::new(),
        ScalarEvidence::new(0.1).unwrap(),
    );
    ctx.emit(RunEvent::PopulationUpdated {
        population_id: frontier.id(),
        events: parent_events,
    });
    let mut parent_selector = TopKParentSelector::best();
    let selected_parent = parent_selector
        .select(frontier)
        .expect("seed candidate initializes the tiny EvoSkill frontier");
    assert_eq!(selected_parent, parent);
    selected_parent
}

fn reflection_input(
    seed: GitProgramArtifact,
    selected_parent: CandidateId,
) -> GitProgramGepaReflectionInput<String> {
    let request = ReflectRequest::for_part(
        selected_parent,
        "repos/program/program.txt".to_owned(),
        "program.txt",
    )
    .with_examples([ReflectiveCase::from_example(
        ReflectiveValue::Text("The program returned the base answer.".to_owned()),
        None,
        Some(ReflectiveValue::Text(
            "The reflected program should change its answer.".to_owned(),
        )),
        Some(0.25),
        "Patch program.txt so the candidate has a new behavior.",
    )])
    .with_source_refs([InfoRef::Candidate(selected_parent)])
    .with_attempt_index(0);
    GitProgramGepaReflectionInput::from_request(seed, request)
}

fn empty_reflection_input(
    seed: GitProgramArtifact,
    selected_parent: CandidateId,
) -> GitProgramGepaReflectionInput<String> {
    let request = ReflectRequest::for_part(
        selected_parent,
        "repos/program/program.txt".to_owned(),
        "program.txt",
    );
    GitProgramGepaReflectionInput::from_request(seed, request)
}

fn materialize_agent_edit_and_parse(
    fixture: &GitFixture,
    input: &GitProgramGepaReflectionInput<String>,
    graph: &RunGraphView<'_, GitProblem>,
) -> ProposalBatch<GitProblem> {
    let materializer =
        GitProgramGepaReflectionMaterializer::new(GitProgramMaterializer::new(fixture.stores()));
    let renderer = GepaGitProgramReflectionRenderer;
    let parser = GitProgramGepaReflectionParser::new(GitProgramReadback::new(fixture.stores()));
    let mut workspace = futures::executor::block_on(
        LocalWorkspaceFactory::temp().allocate(WorkspaceConfig::default()),
    )
    .unwrap();
    let proposal_batch = {
        let mut view = workspace.view();
        let materialized = materializer.materialize_input(input, &mut view).unwrap();
        assert!(materialized.value.files_written >= 2);
        let instructions = renderer.render_input(input).unwrap();
        assert_instruction_context(
            &instructions,
            "GEPA reflection brief",
            ".leaven/gepa-reflection.md",
        );
        assert_instruction_context(&instructions, "repo/program", "repos/program");
        assert_eq!(
            String::from_utf8(
                view.read_file(&workspace_path("repos/program/program.txt"))
                    .unwrap()
            )
            .unwrap(),
            "program base\n"
        );
        assert!(
            String::from_utf8(
                view.read_file(&workspace_path(".leaven/gepa-reflection.md"))
                    .unwrap()
            )
            .unwrap()
            .contains("## Parent Candidate")
        );
        view.write_file(
            &workspace_path("repos/program/program.txt"),
            b"program reflected\n",
        )
        .unwrap();
        parser
            .parse_workspace(&mut view, input, graph)
            .unwrap()
            .value
    };
    futures::executor::block_on(workspace.cleanup()).unwrap();
    proposal_batch
}

fn record_and_apply_child(
    ctx: &mut RunContext<'_, GitProblem>,
    proposal_batch: ProposalBatch<GitProblem>,
) -> CandidateId {
    let proposer = PrebuiltProposalProposer::new(proposal_batch);

    let batch = futures::executor::block_on(ctx.propose(&proposer, ())).unwrap();
    ctx.apply_batch(batch.batch_id)
        .unwrap()
        .successful_candidates()
        .next()
        .unwrap()
}

fn admit_child_to_tiny_frontier(
    ctx: &mut RunContext<'_, GitProblem>,
    frontier: &mut TopKFrontier,
    parent: CandidateId,
    child: CandidateId,
    child_revision: GitRevision,
) -> GitProgramReflectionFrontierReport {
    let child_events = frontier.observe(
        child,
        AssessmentId::new(),
        ScalarEvidence::new(0.9).unwrap(),
    );
    ctx.emit(RunEvent::PopulationUpdated {
        population_id: frontier.id(),
        events: child_events,
    });
    GitProgramReflectionFrontierReport {
        parent,
        child,
        child_revision,
        child_admitted: frontier.contains(child),
        best_candidate: frontier.best(),
        best_score: frontier.best_score(),
    }
}

fn child_revision(ctx: &RunContext<'_, GitProblem>, child: CandidateId) -> GitRevision {
    ctx.graph()
        .artifact(child)
        .unwrap()
        .repo(&repo_key("program"))
        .unwrap()
        .revision()
        .clone()
}

fn assert_child_revision(fixture: &GitFixture, child_revision: &GitRevision) {
    assert_ne!(child_revision, &fixture.program_parent);
    assert_eq!(
        git_output(
            &fixture.program_store,
            [
                "show",
                &format!("{}:program.txt", child_revision.object_id())
            ],
        ),
        "program reflected\n"
    );
}

fn assert_provenance(
    ctx: &RunContext<'_, GitProblem>,
    child: CandidateId,
    selected_parent: CandidateId,
) {
    let proposal = ctx.graph().proposal_that_created(child).unwrap();
    assert!(
        proposal
            .provenance()
            .informed_by_refs()
            .contains(&InfoRef::Candidate(selected_parent)),
        "GEPA Git parser wrapper must preserve reflection provenance"
    );
}

#[derive(Clone, Debug)]
struct GitProblem;

impl OptimizationProblem for GitProblem {
    type Artifact = GitProgramArtifact;
    type Case = ();
    type Evidence = GitEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug, PartialEq)]
struct GitEvidence;

impl Evidence for GitEvidence {}

#[derive(Clone, Debug, PartialEq)]
struct GitProgramReflectionFrontierReport {
    parent: CandidateId,
    child: CandidateId,
    child_revision: GitRevision,
    child_admitted: bool,
    best_candidate: Option<CandidateId>,
    best_score: Option<f64>,
}

#[derive(Clone)]
struct PrebuiltProposalProposer {
    batch: ProposalBatch<GitProblem>,
}

impl PrebuiltProposalProposer {
    fn new(batch: ProposalBatch<GitProblem>) -> Self {
        Self { batch }
    }
}

impl Proposer<GitProblem> for PrebuiltProposalProposer {
    type Request = ();

    fn id(&self) -> ProposerId {
        ProposerId::from("gepa/git-agentic")
    }

    fn arity(&self) -> Arity {
        Arity::Single
    }

    async fn propose(
        &self,
        _request: Self::Request,
        _ctx: ProposalContext<'_, GitProblem>,
    ) -> Result<Metered<ProposalBatch<GitProblem>>, ProposalError> {
        Ok(Metered::new(self.batch.clone(), Cost::zero()))
    }
}

fn assert_instruction_context(instructions: &AgentInstructions, label: &str, path: &str) {
    assert!(
        instructions.context.contains(&AgentContextRef {
            label: label.to_owned(),
            path: workspace_path(path),
            media_type: if Path::new(path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            {
                Some("text/markdown".to_owned())
            } else {
                None
            },
        }),
        "rendered agent instructions should include {label} at {path}"
    );
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under workspace/crates/leaven-gepa-agentic-git")
        .to_path_buf()
}

fn prefixed_candidate_ref(candidate: CandidateId) -> String {
    format!("cand_{candidate}")
}

struct GitFixture {
    program_store: PathBuf,
    program_parent: GitRevision,
    _temp: tempfile::TempDir,
}

impl GitFixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let program_source = temp.path().join("program-source");
        let program_store = temp.path().join("program.git");
        create_repo(&program_source, "program.txt", "program base\n");
        run_git_at(
            temp.path(),
            ["clone", "--bare", "program-source", "program.git"],
        );
        let program_parent = GitRevision::Commit(git_object(
            git_output(&program_source, ["rev-parse", "main"]).trim(),
        ));
        Self {
            program_store,
            program_parent,
            _temp: temp,
        }
    }

    fn stores(&self) -> GitProgramStores {
        GitProgramStores::new(BTreeMap::from([(
            repo_key("program"),
            self.program_store.clone(),
        )]))
        .unwrap()
    }

    fn program_artifact(&self) -> GitProgramArtifact {
        GitProgramArtifact::new(
            BTreeMap::from([(
                repo_key("program"),
                GitRepoArtifact::new(
                    RepoRef::global(repo_key("program")),
                    self.program_parent.clone(),
                    None,
                    GitArtifactIdentityMode::Commit,
                ),
            )]),
            leaven_artifact_git::GitProgramLayout::new(BTreeMap::from([(
                repo_key("program"),
                git_path("repos/program"),
            )]))
            .unwrap(),
        )
        .unwrap()
    }
}

fn create_repo(root: &Path, file: &str, body: &str) {
    fs::create_dir_all(root).unwrap();
    run_git_at(root, ["init", "--initial-branch=main"]);
    run_git_at(root, ["config", "user.name", "Leaven Test"]);
    run_git_at(root, ["config", "user.email", "leaven@example.invalid"]);
    fs::write(root.join(file), body).unwrap();
    run_git_at(root, ["add", file]);
    run_git_at(root, ["commit", "-m", "base"]);
}

fn run_git_at<const N: usize>(cwd: &Path, args: [&str; N]) {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> String {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn repo_key(key: &str) -> RepoKey {
    RepoKey::new(key).unwrap()
}

fn git_path(path: &str) -> GitPath {
    GitPath::new(path).unwrap()
}

fn workspace_path(path: &str) -> WorkspacePath {
    WorkspacePath::new(path).unwrap()
}

fn git_object(hex: &str) -> GitObjectId {
    GitObjectId::new(hex).unwrap()
}
