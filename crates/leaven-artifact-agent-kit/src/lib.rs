//! Repo-backed AgentKit artifact manifest and profile vocabulary.

mod manifest;
mod path;
mod profile;

pub use manifest::{AgentKitManifest, AgentKitManifestError, AgentKitRepoArtifact};
pub use path::{AgentKitPath, AgentKitPathError};
pub use profile::{
    AgentKitProfileCodex, AgentKitProfiles, CodexSystemPromptChannel, HookScaffoldStatus,
};
