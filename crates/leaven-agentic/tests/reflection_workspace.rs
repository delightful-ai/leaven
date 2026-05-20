use std::convert::Infallible;

use futures::executor::block_on;
use leaven_agent::{FakeAgentAction, FakeAgentRuntime};
use leaven_agentic::{
    ArtifactReflector, ReadbackResult, ReflectionLayoutConfig, ReflectionWorkspace,
};
use leaven_kernel::{Budget, BudgetSnapshot};
use leaven_workspace::{WorkspacePath, WorkspaceView};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn reflection_workspace_projects_runs_and_reads_back_current_tree() {
    block_on(async {
        let reflector = TextReflector;
        let runtime = FakeAgentRuntime::new(vec![
            FakeAgentAction::ReadFile {
                path: WorkspacePath::new("MANIFEST.json").unwrap(),
            },
            FakeAgentAction::ReadFile {
                path: WorkspacePath::new("target/current/seed.txt").unwrap(),
            },
            FakeAgentAction::WriteFile {
                path: WorkspacePath::new("target/current/result.txt").unwrap(),
                bytes: b"improved".to_vec(),
            },
        ]);
        let factory = LocalWorkspaceFactory::temp();
        let budget = BudgetSnapshot {
            limit: Budget::unlimited(),
            ..BudgetSnapshot::default()
        };

        let outcome = ReflectionWorkspace::new(ReflectionLayoutConfig::default())
            .run(&reflector, &"seed", &[], &[], &factory, &runtime, &budget)
            .await
            .unwrap();

        assert_eq!(
            outcome.readback,
            ReadbackResult::Valid("improved".to_owned())
        );
        assert!(
            outcome
                .session_attachments
                .iter()
                .any(|attachment| attachment.name == "session/main")
        );
    });
}

struct TextReflector;

impl ArtifactReflector for TextReflector {
    type Input = &'static str;
    type Change = String;
    type Error = Infallible;

    fn reflection_id(&self) -> &'static str {
        "test.text.v1"
    }

    async fn project(
        &self,
        input: &Self::Input,
        view: &mut WorkspaceView<'_>,
    ) -> Result<(), Self::Error> {
        view.write_file(&WorkspacePath::new("seed.txt").unwrap(), input.as_bytes())
            .unwrap();
        Ok(())
    }

    async fn read_back(
        &self,
        _input: &Self::Input,
        view: &WorkspaceView<'_>,
        _session: &leaven_agent::AgentSession,
    ) -> Result<ReadbackResult<Self::Change>, Self::Error> {
        let bytes = view
            .read_file(&WorkspacePath::new("result.txt").unwrap())
            .unwrap();
        Ok(ReadbackResult::Valid(String::from_utf8(bytes).unwrap()))
    }
}
