use std::fs;
use std::path::Path;

use leaven_agentic_agent_kit::{
    AgentKitMountApplied, AgentKitMountMode, CodexAgentKitMaterializer,
};

#[test]
fn materializes_codex_profile_from_repo_subtree() {
    let temp = tempfile::tempdir().unwrap();
    let kit = temp.path().join("repo/agent");
    write_agent_kit(&kit);
    let workspace = temp.path().join("workspace");

    let materializer = CodexAgentKitMaterializer::new(AgentKitMountMode::Copy);
    let materialized = materializer.materialize(&kit, &workspace).unwrap();

    assert_eq!(materialized.system_prompt.as_deref(), Some("Be precise.\n"));
    assert_eq!(
        fs::read_to_string(workspace.join("AGENTS.md")).unwrap(),
        "Use the durable repo identity.\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.join(".agents/skills/alpha/SKILL.md")).unwrap(),
        "---\nname: alpha\ndescription: Alpha skill.\n---\n\nDo alpha work.\n"
    );
    assert!(
        materialized
            .mounts
            .iter()
            .all(|mount| mount.applied == AgentKitMountApplied::Copy)
    );
}

#[test]
fn symlink_preferred_falls_back_to_copy_and_records_it() {
    let temp = tempfile::tempdir().unwrap();
    let kit = temp.path().join("repo/agent");
    write_agent_kit(&kit);
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(workspace.join("AGENTS.md"), "stale\n").unwrap();

    let materializer = CodexAgentKitMaterializer::new(AgentKitMountMode::SymlinkPreferred);
    let materialized = materializer.materialize(&kit, &workspace).unwrap();

    assert_eq!(
        fs::read_to_string(workspace.join("AGENTS.md")).unwrap(),
        "Use the durable repo identity.\n"
    );
    assert!(materialized.mounts.iter().any(|mount| {
        mount.requested == AgentKitMountMode::SymlinkPreferred
            && mount.applied == AgentKitMountApplied::Copy
            && mount.symlink_fallback
    }));
}

fn write_agent_kit(root: &Path) {
    fs::create_dir_all(root.join("skills/alpha")).unwrap();
    fs::write(
        root.join("manifest.toml"),
        r#"
schema = "v1"
system_prompt = "system_prompt.md"
agent_docs = "AGENTS.md"
skills = "skills/"
"#,
    )
    .unwrap();
    fs::write(root.join("system_prompt.md"), "Be precise.\n").unwrap();
    fs::write(root.join("AGENTS.md"), "Use the durable repo identity.\n").unwrap();
    fs::write(
        root.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Alpha skill.\n---\n\nDo alpha work.\n",
    )
    .unwrap();
}
