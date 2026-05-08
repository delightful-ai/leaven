//! Skill-bank workspace proposal parser.

use std::collections::BTreeMap;

use leaven_agent::AgentSession;
use leaven_agentic::{AgenticParseError, ProposalParser};
use leaven_artifact_skill::{
    SkillBank, SkillBankError, SkillFile, SkillFilePermissions, SkillFolder, SkillName, SkillPath,
};
use leaven_core::{OptimizationProblem, Proposal, ProposalBatch, ProposalBatchSemantics};
use leaven_engine::RunGraphView;
use leaven_kernel::{Cost, MetadataBag, Metered};
use leaven_workspace::{WorkspacePath, WorkspaceView};

use crate::{SkillBankDiff, SkillBankProposalInput, SkillWorkspaceLayout};

/// Parses a mutated skill workspace into a `SkillBankChange` proposal.
#[derive(Clone, Debug, Default)]
pub struct SkillBankWorkspaceProposalParser {
    layout: SkillWorkspaceLayout,
}

impl SkillBankWorkspaceProposalParser {
    /// Constructs a parser with an explicit layout.
    #[must_use]
    pub fn new(layout: SkillWorkspaceLayout) -> Self {
        Self { layout }
    }

    /// Returns the layout.
    #[must_use]
    pub const fn layout(&self) -> &SkillWorkspaceLayout {
        &self.layout
    }
}

impl<P> ProposalParser<P, SkillBankProposalInput> for SkillBankWorkspaceProposalParser
where
    P: OptimizationProblem<Artifact = SkillBank>,
    P::ProposalAnnotations: Default,
{
    async fn parse_proposals(
        &self,
        workspace: &mut WorkspaceView<'_>,
        _session: &AgentSession,
        input: &SkillBankProposalInput,
        graph: RunGraphView<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, AgenticParseError> {
        let parent = graph
            .artifact(input.parent)
            .ok_or_else(|| AgenticParseError::Message("parent skill bank not found".to_owned()))?;
        let child = read_skill_bank(workspace, &self.layout)?;
        child
            .validate()
            .map_err(|err| AgenticParseError::with_source("parsed skill bank was invalid", err))?;
        let change = SkillBankDiff::diff(parent, &child).ok_or_else(|| {
            AgenticParseError::Message("skill workspace had no changes".to_owned())
        })?;
        let _verified_child = parent
            .apply_skill_change(&change)
            .expect("skill-bank diff applies to the parent it was computed from");
        let proposal = Proposal::mutate(input.parent, change).build();
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![proposal],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        ))
    }
}

fn read_skill_bank(
    workspace: &WorkspaceView<'_>,
    layout: &SkillWorkspaceLayout,
) -> Result<SkillBank, AgenticParseError> {
    let root = workspace
        .subdir(layout.skills_root.clone())
        .map_err(AgenticParseError::Workspace)?;
    let paths = root
        .list_files(&WorkspacePath::root())
        .map_err(AgenticParseError::Workspace)?;
    let mut grouped: BTreeMap<SkillName, BTreeMap<SkillPath, SkillFile>> = BTreeMap::new();

    for path in paths {
        let (skill_name, skill_path) = skill_path_from_workspace(&path)?;
        let skill_name = SkillName::new(skill_name.to_owned()).map_err(|err| {
            AgenticParseError::with_source("workspace skill folder name was invalid", err)
        })?;
        let bytes = root
            .read_file(&path)
            .map_err(AgenticParseError::Workspace)?;
        let executable = root
            .is_executable(&path)
            .map_err(AgenticParseError::Workspace)?;
        grouped.entry(skill_name).or_default().insert(
            skill_path,
            SkillFile::with_permissions(bytes, SkillFilePermissions { executable }),
        );
    }

    grouped
        .into_iter()
        .map(|(name, entries)| SkillFolder::from_entries(name, entries))
        .collect::<Result<Vec<_>, SkillBankError>>()
        .and_then(SkillBank::from_folders)
        .map_err(|err| {
            AgenticParseError::with_source("workspace did not contain a valid skill bank", err)
        })
}

fn skill_path_from_workspace(path: &WorkspacePath) -> Result<(&str, SkillPath), AgenticParseError> {
    let (skill_name, skill_path) = path.as_str().split_once('/').ok_or_else(|| {
        AgenticParseError::Message(format!(
            "workspace path `{}` is not inside a skill folder",
            path.as_str()
        ))
    })?;
    Ok((
        skill_name,
        SkillPath::new(skill_path.to_owned()).map_err(|err| {
            AgenticParseError::with_source("workspace skill file path was invalid", err)
        })?,
    ))
}
