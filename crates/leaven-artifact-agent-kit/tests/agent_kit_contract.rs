use leaven_artifact_agent_kit::{
    AgentKitManifest, AgentKitManifestError, CodexSystemPromptChannel, HookScaffoldStatus,
};

#[test]
fn parses_manifest_with_system_prompt_agent_docs_and_skills() {
    let manifest = AgentKitManifest::from_toml_str(
        r#"
schema = "v1"
system_prompt = "system_prompt.md"
agent_docs = "AGENTS.md"
skills = "skills/"

[profiles.codex]
system_prompt_channel = "base_instructions"
"#,
    )
    .unwrap();

    assert_eq!(manifest.schema.as_str(), "v1");
    assert_eq!(manifest.system_prompt.unwrap().as_str(), "system_prompt.md");
    assert_eq!(manifest.agent_docs.unwrap().as_str(), "AGENTS.md");
    assert_eq!(manifest.skills.unwrap().as_str(), "skills");
    assert_eq!(
        manifest.profiles.codex.system_prompt_channel,
        CodexSystemPromptChannel::BaseInstructions
    );
}

#[test]
fn rejects_manifest_without_behavior_bearing_slots() {
    let err = AgentKitManifest::from_toml_str(
        r#"
schema = "v1"
hooks = "hooks/"
"#,
    )
    .unwrap_err();

    assert!(matches!(err, AgentKitManifestError::MissingBehaviorSlot));
}

#[test]
fn hooks_are_declared_as_scaffold_only() {
    let manifest = AgentKitManifest::from_toml_str(
        r#"
schema = "v1"
system_prompt = "system_prompt.md"
hooks = "hooks/"
"#,
    )
    .unwrap();

    assert_eq!(
        manifest.hook_status(),
        Some(HookScaffoldStatus::ScaffoldOnly)
    );
}

#[test]
fn rejects_absolute_and_escaping_paths() {
    let absolute = AgentKitManifest::from_toml_str(
        r#"
schema = "v1"
system_prompt = "/tmp/system_prompt.md"
"#,
    )
    .unwrap_err();
    assert!(matches!(
        absolute,
        AgentKitManifestError::InvalidPath { .. }
    ));

    let escaping = AgentKitManifest::from_toml_str(
        r#"
schema = "v1"
system_prompt = "../system_prompt.md"
"#,
    )
    .unwrap_err();
    assert!(matches!(
        escaping,
        AgentKitManifestError::InvalidPath { .. }
    ));
}

#[test]
fn codex_profile_defaults_to_agents_md_and_agents_skills_mounts() {
    let manifest = AgentKitManifest::from_toml_str(
        r#"
schema = "v1"
system_prompt = "system_prompt.md"
"#,
    )
    .unwrap();

    assert_eq!(
        manifest.profiles.codex.agent_docs_mount.as_str(),
        "AGENTS.md"
    );
    assert_eq!(
        manifest.profiles.codex.skills_mount.as_str(),
        ".agents/skills"
    );
    assert_eq!(
        manifest.profiles.codex.system_prompt_channel,
        CodexSystemPromptChannel::BaseInstructions
    );
}

#[test]
fn rejects_unknown_manifest_fields() {
    let err = AgentKitManifest::from_toml_str(
        r#"
schema = "v1"
system_prompt = "system_prompt.md"
provider_flags = ["--dangerous"]
"#,
    )
    .unwrap_err();

    assert!(matches!(err, AgentKitManifestError::Toml(_)));
}
