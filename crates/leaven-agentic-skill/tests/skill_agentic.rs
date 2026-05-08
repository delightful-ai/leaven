use std::collections::BTreeMap;

use leaven_agent::{AgentInstructions, FakeAgentAction, FakeAgentRuntime, OutputContract};
use leaven_agentic::{AgentPromptTarget, AgenticProposer, AgenticProposerConfig, AgenticRunInput};
use leaven_agentic_skill::{
    SkillBankDiff, SkillBankMaterializer, SkillBankProposalInput, SkillBankWorkspaceProposalParser,
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
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text(skill_md("alpha", description, body)),
    );
    entries.insert(
        SkillPath::new("scripts/run.sh").unwrap(),
        SkillFile::with_permissions(
            b"#!/bin/sh\necho alpha\n".to_vec(),
            SkillFilePermissions { executable },
        ),
    );
    SkillBank::from_folders([
        SkillFolder::from_entries(SkillName::new("alpha").unwrap(), entries).unwrap(),
    ])
    .unwrap()
}

fn skill_md(name: &str, description: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n{body}\n")
}
