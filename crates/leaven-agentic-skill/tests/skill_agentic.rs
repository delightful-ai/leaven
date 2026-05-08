use std::collections::BTreeMap;

use leaven_agent::{AgentInstructions, FakeAgentAction, FakeAgentRuntime, OutputContract};
use leaven_agentic::{AgentPromptTarget, AgenticProposer, AgenticProposerConfig, AgenticRunInput};
use leaven_agentic_skill::{
    SkillBankDiff, SkillBankMaterializer, SkillBankProposalInput, SkillBankWorkspaceProposalParser,
    SkillWorkspaceLayout,
};
use leaven_artifact_skill::{
    SkillBank, SkillBankChange, SkillFile, SkillFilePermissions, SkillFolder, SkillName, SkillPath,
};
use leaven_core::{Evidence, OptimizationProblem};
use leaven_engine::{BudgetLedger, RenderContext, RenderError, Renderer, RunContext, RunGraph};
use leaven_kernel::{Cost, Metered, ProposerId, RunId};
use leaven_workspace::WorkspacePath;
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn agentic_proposer_materializes_skill_bank_and_parses_workspace_mutation() {
    futures::executor::block_on(async {
        let seed = bank_with_alpha(
            "Edits Rust tests. Use when Rust test failures need diagnosis.",
            "Read the failing test output, inspect the narrow code path, and patch it.",
            true,
        );
        let mut graph = RunGraph::<SkillProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let parent = {
            let mut ctx = RunContext::<SkillProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(seed, 0).unwrap()
        };
        let proposer = AgenticProposer::new(
            AgenticProposerConfig::new(ProposerId::from("skill/fake")),
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("alpha/SKILL.md").unwrap(),
                bytes: skill_md(
                    "alpha",
                    "Edits Rust tests and fixtures. Use when Rust test failures or fixture drift need diagnosis.",
                    "Read the failing test output, inspect the narrow code path, patch it, and keep fixture changes explicit.",
                )
                .into_bytes(),
            }]),
            SkillBankMaterializer::default(),
            SkillPromptRenderer,
            SkillBankWorkspaceProposalParser::default(),
        );
        let mut ctx = RunContext::<SkillProblem>::new(&mut graph, &mut budget);

        let report = ctx
            .propose(
                &proposer,
                AgenticRunInput::new(
                    SkillBankProposalInput::new(parent),
                    OutputContract::WorkspaceDiff {
                        roots: vec![WorkspacePath::root()],
                    },
                ),
            )
            .await
            .unwrap();
        let applied = ctx.apply_batch(report.batch_id).unwrap();
        let child = applied.successful_candidates().next().unwrap();
        let artifact = ctx.graph().artifact(child).unwrap();
        let alpha = artifact.get(&SkillName::new("alpha").unwrap()).unwrap();

        assert_eq!(
            alpha.manifest().description.as_str(),
            "Edits Rust tests and fixtures. Use when Rust test failures or fixture drift need diagnosis."
        );
        assert_eq!(
            alpha.body().as_str().trim(),
            "Read the failing test output, inspect the narrow code path, patch it, and keep fixture changes explicit."
        );
        assert!(
            alpha
                .file(&SkillPath::new("scripts/run.sh").unwrap())
                .unwrap()
                .permissions()
                .executable
        );
    });
}

#[test]
fn agentic_proposer_supports_nested_skill_layouts() {
    futures::executor::block_on(async {
        let seed = bank_with_alpha(
            "Edits Rust tests. Use when Rust test failures need diagnosis.",
            "Read the failing test output, inspect the narrow code path, and patch it.",
            false,
        );
        let mut graph = RunGraph::<SkillProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let parent = {
            let mut ctx = RunContext::<SkillProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(seed, 0).unwrap()
        };
        let layout = SkillWorkspaceLayout::new(".agents/skills").unwrap();
        let proposer = AgenticProposer::new(
            AgenticProposerConfig::new(ProposerId::from("skill/nested")),
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new(".agents/skills/alpha/SKILL.md").unwrap(),
                bytes: skill_md(
                    "alpha",
                    "Edits nested skill folders. Use when materialized under .agents/skills.",
                    "Patch the nested skill folder while preserving Agent Skills format.",
                )
                .into_bytes(),
            }]),
            SkillBankMaterializer::new(layout.clone()),
            SkillPromptRenderer,
            SkillBankWorkspaceProposalParser::new(layout),
        );
        let mut ctx = RunContext::<SkillProblem>::new(&mut graph, &mut budget);

        let report = ctx
            .propose(
                &proposer,
                AgenticRunInput::new(
                    SkillBankProposalInput::new(parent),
                    OutputContract::WorkspaceDiff {
                        roots: vec![WorkspacePath::new(".agents/skills").unwrap()],
                    },
                ),
            )
            .await
            .unwrap();

        assert_eq!(report.proposal_ids.len(), 1);
    });
}

#[test]
fn skill_bank_materializer_exposes_layout_and_rejects_missing_parent() {
    futures::executor::block_on(async {
        let layout = SkillWorkspaceLayout::new(".agents/skills").unwrap();
        let materializer = SkillBankMaterializer::new(layout.clone());
        assert_eq!(materializer.layout(), &layout);

        let mut graph = RunGraph::<SkillProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let mut ctx = RunContext::<SkillProblem>::new(&mut graph, &mut budget);
        let proposer = AgenticProposer::new(
            AgenticProposerConfig::new(ProposerId::from("skill/missing-parent")),
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(Vec::new()),
            materializer,
            SkillPromptRenderer,
            SkillBankWorkspaceProposalParser::new(layout),
        );
        let missing_parent = leaven_kernel::CandidateId::new();

        let error = ctx
            .propose(
                &proposer,
                AgenticRunInput::new(
                    SkillBankProposalInput::new(missing_parent),
                    OutputContract::WorkspaceDiff {
                        roots: vec![WorkspacePath::new(".agents/skills").unwrap()],
                    },
                ),
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("agentic proposer failed"));
        assert_eq!(ctx.graph().candidate_count(), 0);
    });
}

#[test]
fn agentic_skill_parser_rejects_invalid_mutated_skill_md() {
    futures::executor::block_on(async {
        let seed = bank_with_alpha(
            "Edits Rust tests. Use when Rust test failures need diagnosis.",
            "Read the failing test output and patch the narrow code path.",
            false,
        );
        let mut graph = RunGraph::<SkillProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let parent = {
            let mut ctx = RunContext::<SkillProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(seed, 0).unwrap()
        };
        let proposer = AgenticProposer::new(
            AgenticProposerConfig::new(ProposerId::from("skill/invalid")),
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("alpha/SKILL.md").unwrap(),
                bytes: b"---\nname: alpha\n---\nmissing description".to_vec(),
            }]),
            SkillBankMaterializer::default(),
            SkillPromptRenderer,
            SkillBankWorkspaceProposalParser::default(),
        );
        let mut ctx = RunContext::<SkillProblem>::new(&mut graph, &mut budget);

        let error = ctx
            .propose(
                &proposer,
                AgenticRunInput::new(
                    SkillBankProposalInput::new(parent),
                    OutputContract::WorkspaceDiff {
                        roots: vec![WorkspacePath::root()],
                    },
                ),
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("agentic proposer failed"));
        assert_eq!(ctx.graph().candidate_count(), 1);
    });
}

#[test]
fn agentic_skill_parser_rejects_loose_files_and_invalid_skill_folder_names() {
    futures::executor::block_on(async {
        let loose = propose_with_actions(
            "skill/loose-file",
            vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("loose.md").unwrap(),
                bytes: b"not inside a skill folder".to_vec(),
            }],
        )
        .await
        .unwrap_err();
        assert!(loose.to_string().contains("agentic proposer failed"));

        let invalid_folder = propose_with_actions(
            "skill/invalid-folder",
            vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("Bad/SKILL.md").unwrap(),
                bytes: skill_md("Bad", "Use when invalid names are tested.", "Body.").into_bytes(),
            }],
        )
        .await
        .unwrap_err();
        assert!(
            invalid_folder
                .to_string()
                .contains("agentic proposer failed")
        );
    });
}

#[test]
fn agentic_skill_parser_rejects_skill_relative_paths_invalid_for_skills() {
    futures::executor::block_on(async {
        let error = propose_with_actions(
            "skill/invalid-skill-path",
            vec![
                FakeAgentAction::WriteFile {
                    path: WorkspacePath::new("alpha/SKILL.md").unwrap(),
                    bytes: skill_md(
                        "alpha",
                        "Edits Rust tests. Use when Rust test failures need diagnosis.",
                        "Read the failing test output and patch the narrow code path.",
                    )
                    .into_bytes(),
                },
                FakeAgentAction::WriteFile {
                    path: WorkspacePath::new("alpha/scripts\\run.sh").unwrap(),
                    bytes: b"#!/bin/sh\necho invalid\n".to_vec(),
                },
            ],
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("agentic proposer failed"));
    });
}

#[test]
fn agentic_skill_parser_rejects_unchanged_workspace() {
    futures::executor::block_on(async {
        let seed = bank_with_alpha(
            "Edits Rust tests. Use when Rust test failures need diagnosis.",
            "Read the failing test output and patch the narrow code path.",
            false,
        );
        let mut graph = RunGraph::<SkillProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let parent = {
            let mut ctx = RunContext::<SkillProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(seed, 0).unwrap()
        };
        let proposer = AgenticProposer::new(
            AgenticProposerConfig::new(ProposerId::from("skill/unchanged")),
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(Vec::new()),
            SkillBankMaterializer::default(),
            SkillPromptRenderer,
            SkillBankWorkspaceProposalParser::default(),
        );
        let mut ctx = RunContext::<SkillProblem>::new(&mut graph, &mut budget);

        let error = ctx
            .propose(
                &proposer,
                AgenticRunInput::new(
                    SkillBankProposalInput::new(parent),
                    OutputContract::WorkspaceDiff {
                        roots: vec![WorkspacePath::root()],
                    },
                ),
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("agentic proposer failed"));
        assert_eq!(ctx.graph().candidate_count(), 1);
    });
}

#[test]
fn skill_bank_diff_emits_file_level_changes_for_stable_skill_names() {
    let parent = bank_with_alpha(
        "Edits Rust tests. Use when Rust test failures need diagnosis.",
        "Read the failing test output and patch the narrow code path.",
        false,
    );
    let child = bank_with_alpha(
        "Edits Rust tests and fixtures. Use when Rust test failures or fixture drift need diagnosis.",
        "Read the failing test output and patch the narrow code path.",
        true,
    );

    let change = SkillBankDiff::diff(&parent, &child).unwrap();
    let SkillBankChange::Atomic(changes) = change else {
        panic!("expected atomic file-level diff: {change:?}");
    };

    assert!(matches!(
        &changes[0],
        SkillBankChange::WriteFile { path, .. } if path.is_skill_md()
    ));
    assert!(matches!(
        &changes[1],
        SkillBankChange::SetExecutable {
            path,
            executable: true,
            ..
        } if path.as_str() == "scripts/run.sh"
    ));
}

#[test]
fn skill_bank_diff_handles_create_remove_noop_and_mentions() {
    let parent = bank_with_alpha(
        "Edits Rust tests. Use when Rust test failures need diagnosis.",
        "Read the failing test output and patch the narrow code path.",
        false,
    );
    assert!(SkillBankDiff::diff(&parent, &parent).is_none());

    let child = SkillBank::from_folders([
        parent
            .get(&SkillName::new("alpha").unwrap())
            .unwrap()
            .clone(),
        folder_for(
            "beta",
            "Reviews generated skills. Use when a proposed skill needs validation.",
            "Inspect the generated skill and report concrete schema problems.",
            false,
        ),
    ])
    .unwrap();
    let create = SkillBankDiff::diff(&parent, &child).unwrap();
    assert!(matches!(create, SkillBankChange::CreateSkill { .. }));
    assert!(SkillBankDiff::mentions(
        &create,
        &SkillName::new("beta").unwrap()
    ));

    let empty = SkillBank::default();
    let remove = SkillBankDiff::diff(&parent, &empty).unwrap();
    assert!(matches!(remove, SkillBankChange::RemoveSkill { .. }));
    assert!(SkillBankDiff::mentions(
        &remove,
        &SkillName::new("alpha").unwrap()
    ));
}

#[test]
fn skill_bank_diff_handles_file_add_remove_and_all_mention_variants() {
    let parent = bank_with_alpha(
        "Edits Rust tests. Use when Rust test failures need diagnosis.",
        "Read the failing test output and patch the narrow code path.",
        false,
    );
    let alpha = SkillName::new("alpha").unwrap();
    let child_without_script = SkillBank::from_folders([folder_without_script(
        "alpha",
        "Edits Rust tests. Use when Rust test failures need diagnosis.",
        "Read the failing test output and patch the narrow code path.",
    )])
    .unwrap();
    let remove_file = SkillBankDiff::diff(&parent, &child_without_script).unwrap();
    assert!(matches!(remove_file, SkillBankChange::RemoveFile { .. }));
    assert!(SkillBankDiff::mentions(&remove_file, &alpha));

    let mut extra_entries = folder_for(
        "alpha",
        "Edits Rust tests. Use when Rust test failures need diagnosis.",
        "Read the failing test output and patch the narrow code path.",
        false,
    )
    .entries()
    .clone();
    extra_entries.insert(
        SkillPath::new("references/example.md").unwrap(),
        SkillFile::text("Example.\n"),
    );
    let child_with_extra =
        SkillBank::from_folders([SkillFolder::from_entries(alpha.clone(), extra_entries).unwrap()])
            .unwrap();
    let write_file = SkillBankDiff::diff(&parent, &child_with_extra).unwrap();
    assert!(matches!(write_file, SkillBankChange::WriteFile { .. }));
    assert!(SkillBankDiff::mentions(&write_file, &alpha));

    let rename = SkillBankChange::RenameSkill {
        from: alpha.clone(),
        to: SkillName::new("beta").unwrap(),
    };
    assert!(SkillBankDiff::mentions(&rename, &alpha));
    let rename_file = SkillBankChange::RenameFile {
        skill: alpha.clone(),
        from: SkillPath::new("scripts/run.sh").unwrap(),
        to: SkillPath::new("scripts/renamed.sh").unwrap(),
    };
    assert!(SkillBankDiff::mentions(&rename_file, &alpha));
    let atomic = SkillBankChange::Atomic(vec![rename, write_file]);
    assert!(SkillBankDiff::mentions(&atomic, &alpha));
}

struct SkillPromptRenderer;

impl Renderer<SkillProblem, SkillBankProposalInput, AgentPromptTarget> for SkillPromptRenderer {
    type View = AgentInstructions;

    async fn render(
        &self,
        _value: &SkillBankProposalInput,
        _target: AgentPromptTarget,
        _ctx: RenderContext<'_, SkillProblem>,
    ) -> Result<Metered<Self::View>, RenderError> {
        Ok(Metered::new(
            AgentInstructions::task("improve the alpha skill"),
            Cost::zero(),
        ))
    }
}

struct SkillProblem;

impl OptimizationProblem for SkillProblem {
    type Artifact = SkillBank;
    type Case = ();
    type Evidence = SkillEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug)]
struct SkillEvidence;

impl Evidence for SkillEvidence {}

fn bank_with_alpha(description: &str, body: &str, executable: bool) -> SkillBank {
    SkillBank::from_folders([folder_for("alpha", description, body, executable)]).unwrap()
}

async fn propose_with_actions(
    proposer_id: &str,
    actions: Vec<FakeAgentAction>,
) -> Result<leaven_engine::ProposalBatchReport, leaven_engine::RunContextError> {
    let seed = bank_with_alpha(
        "Edits Rust tests. Use when Rust test failures need diagnosis.",
        "Read the failing test output and patch the narrow code path.",
        false,
    );
    let mut graph = RunGraph::<SkillProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let parent = {
        let mut ctx = RunContext::<SkillProblem>::new(&mut graph, &mut budget);
        ctx.insert_seed(seed, 0).unwrap()
    };
    let parser = SkillBankWorkspaceProposalParser::default();
    assert_eq!(parser.layout(), &SkillWorkspaceLayout::root());
    let proposer = AgenticProposer::new(
        AgenticProposerConfig::new(ProposerId::from(proposer_id.to_owned())),
        LocalWorkspaceFactory::temp(),
        FakeAgentRuntime::new(actions),
        SkillBankMaterializer::default(),
        SkillPromptRenderer,
        parser,
    );
    let mut ctx = RunContext::<SkillProblem>::new(&mut graph, &mut budget);
    ctx.propose(
        &proposer,
        AgenticRunInput::new(
            SkillBankProposalInput::new(parent),
            OutputContract::WorkspaceDiff {
                roots: vec![WorkspacePath::root()],
            },
        ),
    )
    .await
}

fn folder_for(name: &str, description: &str, body: &str, executable: bool) -> SkillFolder {
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text(skill_md(name, description, body)),
    );
    entries.insert(
        SkillPath::new("scripts/run.sh").unwrap(),
        SkillFile::with_permissions(
            b"#!/bin/sh\necho alpha\n".to_vec(),
            SkillFilePermissions { executable },
        ),
    );
    SkillFolder::from_entries(SkillName::new(name).unwrap(), entries).unwrap()
}

fn folder_without_script(name: &str, description: &str, body: &str) -> SkillFolder {
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text(skill_md(name, description, body)),
    );
    SkillFolder::from_entries(SkillName::new(name).unwrap(), entries).unwrap()
}

fn skill_md(name: &str, description: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n{body}\n")
}
