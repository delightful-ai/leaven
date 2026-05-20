use std::collections::BTreeMap;

use leaven_agent::{FakeAgentAction, FakeAgentRuntime};
use leaven_agentic::AgenticProposerConfig;
use leaven_agentic_skill::SkillWorkspaceLayout;
use leaven_artifact_skill::{
    SkillBank, SkillFile, SkillFilePartId, SkillFileSurface, SkillFolder, SkillName, SkillPath,
};
use leaven_core::{Evidence, InfoRef, OptimizationProblem};
use leaven_engine::{BudgetLedger, RunContext, RunGraph};
use leaven_gepa::{GepaReflector, ReflectRequest, ReflectiveCase, ReflectiveValue};
use leaven_gepa_agentic_skill::GepaSkillBankAgenticReflector;
use leaven_kernel::{ProposerId, RunId};
use leaven_workspace::WorkspacePath;
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn skill_bank_gepa_reflector_materializes_agent_edit_and_applies_child() {
    futures::executor::block_on(async {
        let seed = bank_with_alpha(
            "Debugs Rust tests. Use when Rust tests fail.",
            "Read the failing output and patch the narrow Rust test path.",
        );
        let mut graph = RunGraph::<SkillProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let parent = {
            let mut ctx = RunContext::<SkillProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(seed, 0).unwrap()
        };
        let layout = SkillWorkspaceLayout::new(".agents/skills").unwrap();
        let mut reflector = GepaSkillBankAgenticReflector::new(
            AgenticProposerConfig::new(ProposerId::from("gepa/skill-agentic")),
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![
                FakeAgentAction::ReadFile {
                    path: WorkspacePath::new("target/current/.agents/skills/alpha/SKILL.md")
                        .unwrap(),
                },
                FakeAgentAction::WriteFile {
                    path: WorkspacePath::new("target/current/.agents/skills/alpha/SKILL.md")
                        .unwrap(),
                    bytes: skill_md(
                        "alpha",
                        "Debugs Rust tests and fixtures. Use when Rust tests fail or fixture drift appears.",
                        "Read the failing output, inspect the narrow Rust path, and keep fixture edits explicit.",
                    )
                    .into_bytes(),
                },
            ]),
            layout,
        );
        let part = SkillFilePartId {
            skill: SkillName::new("alpha").unwrap(),
            path: SkillPath::skill_md(),
        };
        let request = ReflectRequest::for_part(parent, part, "alpha/SKILL.md")
            .with_examples([ReflectiveCase::from_example(
                ReflectiveValue::Text(
                    "cargo nextest run -p leaven-gepa-agentic-skill failed".to_owned(),
                ),
                None,
                Some(ReflectiveValue::Text(
                    "The prior skill ignored fixture drift.".to_owned(),
                )),
                Some(0.25),
                "Mention fixture drift and require explicit fixture edits.",
            )])
            .with_source_refs([InfoRef::Candidate(parent)])
            .with_attempt_index(0);
        let mut ctx = RunContext::<SkillProblem>::new(&mut graph, &mut budget);

        let child = reflector
            .reflect_candidate(&mut ctx, &SkillFileSurface, request)
            .await
            .unwrap()
            .unwrap();

        let child_bank = ctx.graph().artifact(child).unwrap();
        let alpha = child_bank.get(&SkillName::new("alpha").unwrap()).unwrap();
        assert_eq!(
            alpha.manifest().description.as_str(),
            "Debugs Rust tests and fixtures. Use when Rust tests fail or fixture drift appears."
        );
        assert_eq!(
            alpha.body().as_str().trim(),
            "Read the failing output, inspect the narrow Rust path, and keep fixture edits explicit."
        );
        assert_eq!(ctx.graph().parents(child), vec![parent]);
        let proposal = ctx.graph().proposal_that_created(child).unwrap();
        assert!(
            proposal
                .provenance()
                .informed_by_refs()
                .contains(&InfoRef::Candidate(parent)),
            "GEPA parser wrapper must preserve reflection provenance"
        );
    });
}

#[derive(Clone, Debug)]
struct SkillProblem;

impl OptimizationProblem for SkillProblem {
    type Artifact = SkillBank;
    type Case = ();
    type Evidence = NoEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug)]
struct NoEvidence;

impl Evidence for NoEvidence {}

fn bank_with_alpha(description: &str, body: &str) -> SkillBank {
    let name = SkillName::new("alpha").unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text(skill_md("alpha", description, body)),
    );
    SkillBank::from_folders([SkillFolder::from_entries(name, entries).unwrap()]).unwrap()
}

fn skill_md(name: &str, description: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n{body}\n")
}
