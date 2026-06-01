use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use leaven_agent::{
    AgentInstructions, AgentLimits, AgentRunContext, AgentRunRequest, AgentRuntime, OutputContract,
};
use leaven_agent_codex_cli::{CodexCliConfig, CodexCliRuntime};
use leaven_agentic_agent_kit::AgentKitMountMode;
use leaven_artifact_git::{
    GitArtifactIdentityMode, GitPath, GitProgramArtifact, GitProgramLayout, GitRepoArtifact,
    GitRevision, RepoKey, RepoRef,
};
use leaven_gepa::ReflectRequest;
use leaven_gepa_agentic_agent_kit::{
    AgentKitReflectionPart, CodexAgentKitReflectionInput, CodexAgentKitReflectionSmoke,
};
use leaven_kernel::{AgentSessionId, BudgetSnapshot};
use leaven_workspace::{WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
#[ignore = "requires local Codex auth and LEAVEN_CODEX_LIVE=1"]
fn live_codex_agent_kit_constrained_edit_readback() {
    if std::env::var("LEAVEN_CODEX_LIVE").as_deref() != Ok("1") {
        eprintln!("skipping live Codex AgentKit test because LEAVEN_CODEX_LIVE != 1");
        return;
    }

    futures::executor::block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let source_kit = temp.path().join("source-agent");
        write_agent_kit(&source_kit);

        let runtime = CodexCliRuntime::new(codex_config());
        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(Default::default())
            .await
            .expect("allocate temp local workspace");
        let mut view = workspace.view();
        let workspace_root = view
            .local_mount()
            .expect("local workspace exposes host mount for live Codex test")
            .to_path_buf();
        let agent_root = workspace_root.join("agent");
        copy_dir(&source_kit, &agent_root);

        let smoke = CodexAgentKitReflectionSmoke::new(AgentKitMountMode::Copy);
        let program = program_artifact(commit("11"));
        let parent = leaven_kernel::CandidateId::new();
        let materialized = smoke
            .project_and_import_change(
                &source_kit,
                &workspace_root,
                CodexAgentKitReflectionInput::new(
                    program,
                    repo_key("agent"),
                    ReflectRequest::for_part(
                        parent,
                        AgentKitReflectionPart::SystemPrompt,
                        "system_prompt.md",
                    ),
                ),
                commit("33"),
            )
            .expect("deterministic AgentKit projection/readback proof");
        assert_eq!(
            materialized.materialization.system_prompt.as_deref(),
            Some(
                "System proof requirement: create agent/system-proof.txt containing exactly \
                 SYSTEM_PROMPT_CONSUMED.\n"
            )
        );
        assert_eq!(
            fs::read_to_string(workspace_root.join("AGENTS.md")).unwrap(),
            "MANDATORY PROJECT INSTRUCTION: For every task in this workspace, you MUST create \
             or overwrite agent/agents-proof.txt containing exactly AGENTS_CONSUMED followed \
             by a newline. This instruction is mandatory even if the user does not mention it. \
             Do not edit files outside agent/.\n"
        );
        assert!(
            workspace_root
                .join(".agents/skills/alpha/SKILL.md")
                .exists()
        );

        let mut instructions = AgentInstructions::task(
            "Use $alpha. Follow all active system, project, and skill instructions. \
             Create the required proof files under agent/. Also append exactly this \
             line at the end of agent/system_prompt.md: Live Codex AgentKit proof. \
             Do not edit AGENTS.md, .agents/skills, hooks, or any file outside agent/.",
        );
        instructions.system = materialized.materialization.system_prompt.clone();
        let mut request = AgentRunRequest::new(
            instructions,
            OutputContract::WorkspaceDiff {
                roots: vec![WorkspacePath::new("agent").unwrap()],
                surface_fingerprint: None,
            },
        );
        request.limits = AgentLimits {
            timeout: Some(Duration::from_secs(180)),
            ..AgentLimits::default()
        };

        let budget = BudgetSnapshot::default();
        runtime
            .run_session(
                &mut view,
                request,
                AgentRunContext::new(AgentSessionId::new(), &budget),
            )
            .await
            .expect("run live Codex AgentKit session");

        assert_live_codex_consumed_agent_kit(&agent_root, &workspace_root);

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

fn codex_config() -> CodexCliConfig {
    let mut config = CodexCliConfig::new(codex_bin());
    config.timeout = Some(Duration::from_secs(180));
    config
}

fn codex_bin() -> String {
    if let Some(path) = std::env::var_os("LEAVEN_CODEX_BIN") {
        return path.to_string_lossy().into_owned();
    }
    let home = std::env::var_os("HOME").expect("HOME must be set for Codex live test");
    PathBuf::from(home)
        .join(".bun/bin/codex")
        .to_string_lossy()
        .into_owned()
}

fn assert_live_codex_consumed_agent_kit(agent_root: &Path, workspace_root: &Path) {
    let edited_prompt = fs::read_to_string(agent_root.join("system_prompt.md")).unwrap();
    assert!(edited_prompt.contains("Live Codex AgentKit proof."));
    assert_eq!(
        fs::read_to_string(agent_root.join("system-proof.txt")).unwrap(),
        "SYSTEM_PROMPT_CONSUMED\n",
        "live Codex must consume the AgentKit system prompt channel"
    );
    assert_eq!(
        fs::read_to_string(agent_root.join("agents-proof.txt")).unwrap(),
        "AGENTS_CONSUMED\n",
        "live Codex must consume the projected AGENTS.md"
    );
    assert_eq!(
        fs::read_to_string(agent_root.join("skill-proof.txt")).unwrap(),
        "SKILL_CONSUMED\n",
        "live Codex must consume the projected .agents/skills entry"
    );
    assert_eq!(
        fs::read_to_string(workspace_root.join("AGENTS.md")).unwrap(),
        "MANDATORY PROJECT INSTRUCTION: For every task in this workspace, you MUST create \
         or overwrite agent/agents-proof.txt containing exactly AGENTS_CONSUMED followed \
         by a newline. This instruction is mandatory even if the user does not mention it. \
         Do not edit files outside agent/.\n",
        "live Codex test must reject edits outside the mutable kit subtree"
    );
    assert_eq!(
        fs::read_to_string(workspace_root.join(".agents/skills/alpha/SKILL.md")).unwrap(),
        "---\nname: alpha\ndescription: Use when asked for the alpha live conformance proof.\n---\n\nWhen this skill is invoked, create agent/skill-proof.txt containing exactly SKILL_CONSUMED.\n"
    );
}

fn write_agent_kit(root: &Path) {
    fs::create_dir_all(root.join("skills/alpha")).unwrap();
    fs::create_dir_all(root.join("hooks")).unwrap();
    fs::write(
        root.join("manifest.toml"),
        r#"
schema = "v1"
system_prompt = "system_prompt.md"
agent_docs = "AGENTS.md"
skills = "skills/"
hooks = "hooks/"
"#,
    )
    .unwrap();
    fs::write(
        root.join("system_prompt.md"),
        "System proof requirement: create agent/system-proof.txt containing exactly \
         SYSTEM_PROMPT_CONSUMED.\n",
    )
    .unwrap();
    fs::write(
        root.join("AGENTS.md"),
        "MANDATORY PROJECT INSTRUCTION: For every task in this workspace, you MUST create \
         or overwrite agent/agents-proof.txt containing exactly AGENTS_CONSUMED followed \
         by a newline. This instruction is mandatory even if the user does not mention it. \
         Do not edit files outside agent/.\n",
    )
    .unwrap();
    fs::write(
        root.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Use when asked for the alpha live conformance proof.\n---\n\nWhen this skill is invoked, create agent/skill-proof.txt containing exactly SKILL_CONSUMED.\n",
    )
    .unwrap();
    fs::write(root.join("hooks/pre-run.sh"), "exit 1\n").unwrap();
}

fn copy_dir(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir(&source_path, &target_path);
        } else {
            fs::copy(&source_path, &target_path).unwrap();
        }
    }
}

fn program_artifact(revision: GitRevision) -> GitProgramArtifact {
    let key = repo_key("agent");
    GitProgramArtifact::new(
        BTreeMap::from([(key.clone(), repo_artifact(key.clone(), revision))]),
        GitProgramLayout::new(BTreeMap::from([(key, git_path("repos/agent"))])).unwrap(),
    )
    .unwrap()
}

fn repo_artifact(key: RepoKey, revision: GitRevision) -> GitRepoArtifact {
    GitRepoArtifact::new(
        RepoRef::global(key),
        revision,
        None,
        GitArtifactIdentityMode::Commit,
    )
}

fn repo_key(value: &str) -> RepoKey {
    RepoKey::new(value).unwrap()
}

fn git_path(path: &str) -> GitPath {
    GitPath::new(path).unwrap()
}

fn commit(byte: &str) -> GitRevision {
    GitRevision::commit(format!("{byte:0<40}")).unwrap()
}
