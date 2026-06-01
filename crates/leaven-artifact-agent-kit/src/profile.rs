use crate::AgentKitPath;

/// Provider profile projections for an `AgentKit`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AgentKitProfiles {
    /// Codex workspace projection profile.
    pub codex: AgentKitProfileCodex,
}

/// Codex projection settings for a provider-neutral `AgentKit`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentKitProfileCodex {
    /// Where `system_prompt.md` is supplied to Codex.
    pub system_prompt_channel: CodexSystemPromptChannel,
    /// Workspace path where agent-facing docs are mounted.
    pub agent_docs_mount: AgentKitPath,
    /// Workspace path where skill folders are mounted.
    pub skills_mount: AgentKitPath,
}

impl Default for AgentKitProfileCodex {
    fn default() -> Self {
        Self {
            system_prompt_channel: CodexSystemPromptChannel::BaseInstructions,
            agent_docs_mount: AgentKitPath::new("AGENTS.md")
                .expect("default Codex AGENTS.md mount is valid"),
            skills_mount: AgentKitPath::new(".agents/skills")
                .expect("default Codex skills mount is valid"),
        }
    }
}

/// Channel used for the Codex system prompt projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexSystemPromptChannel {
    /// Project `system_prompt.md` into the base instruction channel.
    BaseInstructions,
    /// Project `system_prompt.md` into a stdin preamble.
    StdinPreamble,
}

/// Hook declarations are recognized but not executable in this slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookScaffoldStatus {
    /// The kit declares a hooks slot, but no execution semantics exist.
    ScaffoldOnly,
}
