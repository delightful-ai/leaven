use std::fs;
use std::path::Path;

use leaven_agentic_agent_kit::{
    AgentKitMountMode, CodexAgentKitMaterialization, CodexAgentKitMaterializer,
    CodexAgentKitMaterializerError,
};
use leaven_artifact_agent_kit::{AgentKitManifest, AgentKitManifestError};
use leaven_artifact_git::{
    GitArtifactError, GitProgramArtifact, GitProgramChange, GitRevision, RepoKey,
};
use leaven_core::Artifact;
use leaven_gepa::ReflectRequest;

/// AgentKit part targeted by a GEPA reflection request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentKitReflectionPart {
    /// The `system_prompt.md` slot.
    SystemPrompt,
    /// The `AGENTS.md` slot.
    AgentDocs,
    /// One named skill folder.
    Skill { name: String },
}

/// Inputs for the deterministic Codex AgentKit reflection smoke.
#[derive(Clone, Debug)]
pub struct CodexAgentKitReflectionInput {
    pub artifact: GitProgramArtifact,
    pub repo: RepoKey,
    pub request: ReflectRequest<AgentKitReflectionPart>,
}

impl CodexAgentKitReflectionInput {
    /// Constructs a deterministic reflection smoke input.
    #[must_use]
    pub const fn new(
        artifact: GitProgramArtifact,
        repo: RepoKey,
        request: ReflectRequest<AgentKitReflectionPart>,
    ) -> Self {
        Self {
            artifact,
            repo,
            request,
        }
    }
}

/// Deterministic report for the provider-free AgentKit reflection smoke.
#[derive(Clone, Debug)]
pub struct CodexAgentKitReflectionReport {
    /// Codex profile projection report.
    pub materialization: CodexAgentKitMaterialization,
    /// Typed Git program change imported from the reflected kit revision.
    pub change: GitProgramChange,
    /// Whether the reflection targeted `system_prompt.md`.
    pub system_prompt_targeted: bool,
    /// Whether the reflection targeted `AGENTS.md`.
    pub agent_docs_targeted: bool,
    /// Whether a declared hook scaffold was recognized and ignored.
    pub hook_scaffold_ignored: bool,
}

/// Provider-free smoke bridge for Codex AgentKit GEPA reflection.
#[derive(Clone, Copy, Debug)]
pub struct CodexAgentKitReflectionSmoke {
    materializer: CodexAgentKitMaterializer,
}

impl CodexAgentKitReflectionSmoke {
    /// Constructs a smoke bridge with the requested materialization policy.
    #[must_use]
    pub const fn new(mount_mode: AgentKitMountMode) -> Self {
        Self {
            materializer: CodexAgentKitMaterializer::new(mount_mode),
        }
    }

    /// Projects the AgentKit and imports a typed child Git revision change.
    ///
    /// This is deterministic proof plumbing. The caller supplies the child
    /// revision that a real Git readback adapter would discover after a
    /// provider run.
    ///
    /// # Errors
    ///
    /// Returns [`CodexAgentKitReflectionSmokeError`] when manifest loading,
    /// materialization, repo lookup, or typed Git change validation fails.
    pub fn project_and_import_change(
        &self,
        kit_root: impl AsRef<Path>,
        workspace_root: impl AsRef<Path>,
        input: CodexAgentKitReflectionInput,
        child: GitRevision,
    ) -> Result<CodexAgentKitReflectionReport, CodexAgentKitReflectionSmokeError> {
        let kit_root = kit_root.as_ref();
        let manifest = load_manifest(kit_root)?;
        let materialization = self.materializer.materialize(kit_root, workspace_root)?;
        let parent_revision = input
            .artifact
            .repo(&input.repo)
            .ok_or_else(|| CodexAgentKitReflectionSmokeError::MissingRepo {
                repo: input.repo.clone(),
            })?
            .revision()
            .clone();
        let change = GitProgramChange::AdvanceRepo {
            repo: input.repo,
            expected_parent: parent_revision,
            child,
        };
        let _verified_child = input.artifact.apply_change(&change)?;

        Ok(CodexAgentKitReflectionReport {
            materialization,
            change,
            system_prompt_targeted: input.request.part == AgentKitReflectionPart::SystemPrompt,
            agent_docs_targeted: input.request.part == AgentKitReflectionPart::AgentDocs,
            hook_scaffold_ignored: manifest.hook_status().is_some(),
        })
    }
}

/// Provider-free AgentKit reflection smoke failure.
#[derive(Debug, thiserror::Error)]
pub enum CodexAgentKitReflectionSmokeError {
    /// The AgentKit manifest could not be read.
    #[error("failed to read AgentKit manifest at {path}")]
    ReadManifest {
        /// Manifest path.
        path: std::path::PathBuf,
        /// Underlying I/O failure.
        #[source]
        source: std::io::Error,
    },
    /// The AgentKit manifest is invalid.
    #[error("invalid AgentKit manifest")]
    Manifest(#[from] AgentKitManifestError),
    /// The Codex profile projection failed.
    #[error("Codex AgentKit materialization failed")]
    Materialize(#[from] CodexAgentKitMaterializerError),
    /// The requested Git program repo is absent from the parent artifact.
    #[error("Git program repo {repo} is missing")]
    MissingRepo {
        /// Missing repo key.
        repo: RepoKey,
    },
    /// The imported Git program change did not apply to the parent artifact.
    #[error("Git program change did not apply")]
    Git(#[from] GitArtifactError),
}

fn load_manifest(root: &Path) -> Result<AgentKitManifest, CodexAgentKitReflectionSmokeError> {
    let path = root.join("manifest.toml");
    let text =
        fs::read_to_string(&path).map_err(|source| CodexAgentKitReflectionSmokeError::ReadManifest {
            path: path.clone(),
            source,
        })?;
    Ok(AgentKitManifest::from_toml_str(&text)?)
}
