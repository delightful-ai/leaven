use leaven_artifact_git::{GitProgramArtifact, GitRepoArtifact};

use crate::{AgentKitPath, AgentKitPathError, AgentKitProfiles, HookScaffoldStatus};

/// Parsed `manifest.toml` for a repo-backed `AgentKit`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentKitManifest {
    pub schema: AgentKitSchema,
    pub system_prompt: Option<AgentKitPath>,
    pub agent_docs: Option<AgentKitPath>,
    pub skills: Option<AgentKitPath>,
    pub hooks: Option<AgentKitPath>,
    pub harness: Option<AgentKitPath>,
    pub profiles: AgentKitProfiles,
}

impl AgentKitManifest {
    /// Parses and validates an `AgentKit` manifest from TOML.
    ///
    /// # Errors
    ///
    /// Returns [`AgentKitManifestError`] when TOML is malformed, unknown fields
    /// are present, paths are invalid, or no behavior-bearing slot is declared.
    pub fn from_toml_str(input: &str) -> Result<Self, AgentKitManifestError> {
        let raw = toml::from_str::<RawManifest>(input).map_err(AgentKitManifestError::Toml)?;

        let system_prompt = parse_slot("system_prompt", raw.system_prompt)?;
        let agent_docs = parse_slot("agent_docs", raw.agent_docs)?;
        let skills = parse_slot("skills", raw.skills)?;
        let hooks = parse_slot("hooks", raw.hooks)?;
        let harness = parse_slot("harness", raw.harness)?;

        if system_prompt.is_none() && agent_docs.is_none() && skills.is_none() && harness.is_none()
        {
            return Err(AgentKitManifestError::MissingBehaviorSlot);
        }

        Ok(Self {
            schema: raw.schema,
            system_prompt,
            agent_docs,
            skills,
            hooks,
            harness,
            profiles: raw.profiles.unwrap_or_default().try_into()?,
        })
    }

    /// Returns hook status when the manifest declares the hook scaffold slot.
    pub const fn hook_status(&self) -> Option<HookScaffoldStatus> {
        if self.hooks.is_some() {
            Some(HookScaffoldStatus::ScaffoldOnly)
        } else {
            None
        }
    }
}

/// Repo artifact identity that backs an `AgentKit` view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentKitRepoArtifact {
    /// Single repository revision.
    Repo(GitRepoArtifact),
    /// Multi-repository Git program revision.
    Program(GitProgramArtifact),
}

/// `AgentKit` manifest schema version.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKitSchema {
    /// Initial repo-backed `AgentKit` manifest schema.
    V1,
}

impl AgentKitSchema {
    /// Returns the schema string used in `manifest.toml`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "v1",
        }
    }
}

/// Manifest parse or validation failure.
#[derive(Debug, thiserror::Error)]
pub enum AgentKitManifestError {
    /// The TOML document is malformed or contains unknown fields.
    #[error("invalid AgentKit manifest TOML")]
    Toml(#[source] toml::de::Error),
    /// A manifest slot path is invalid.
    #[error("invalid AgentKit path in field {field}: {path}")]
    InvalidPath {
        /// Manifest field that carried the path.
        field: &'static str,
        /// Original path value.
        path: String,
        /// Path validation failure.
        #[source]
        source: AgentKitPathError,
    },
    /// At least one behavior-bearing slot is required.
    #[error("AgentKit manifest must declare system_prompt, agent_docs, skills, or harness")]
    MissingBehaviorSlot,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema: AgentKitSchema,
    system_prompt: Option<String>,
    agent_docs: Option<String>,
    skills: Option<String>,
    hooks: Option<String>,
    harness: Option<String>,
    profiles: Option<RawProfiles>,
}

#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProfiles {
    codex: Option<RawCodexProfile>,
}

impl TryFrom<RawProfiles> for AgentKitProfiles {
    type Error = AgentKitManifestError;

    fn try_from(value: RawProfiles) -> Result<Self, Self::Error> {
        Ok(Self {
            codex: value.codex.unwrap_or_default().try_into()?,
        })
    }
}

#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCodexProfile {
    system_prompt_channel: Option<crate::CodexSystemPromptChannel>,
    agent_docs_mount: Option<String>,
    skills_mount: Option<String>,
}

impl TryFrom<RawCodexProfile> for crate::AgentKitProfileCodex {
    type Error = AgentKitManifestError;

    fn try_from(value: RawCodexProfile) -> Result<Self, Self::Error> {
        let defaults = Self::default();
        Ok(Self {
            system_prompt_channel: value
                .system_prompt_channel
                .unwrap_or(defaults.system_prompt_channel),
            agent_docs_mount: parse_slot(
                "profiles.codex.agent_docs_mount",
                value.agent_docs_mount,
            )?
            .unwrap_or(defaults.agent_docs_mount),
            skills_mount: parse_slot("profiles.codex.skills_mount", value.skills_mount)?
                .unwrap_or(defaults.skills_mount),
        })
    }
}

fn parse_slot(
    field: &'static str,
    value: Option<String>,
) -> Result<Option<AgentKitPath>, AgentKitManifestError> {
    value
        .map(|path| {
            AgentKitPath::new(path.clone()).map_err(|source| AgentKitManifestError::InvalidPath {
                field,
                path,
                source,
            })
        })
        .transpose()
}
