use std::convert::Infallible;

use futures::executor::block_on;
use leaven_agent::{AgentStatus, FakeAgentAction, FakeAgentRuntime};
use leaven_agentic::{
    ArtifactReflector, ReadbackResult, ReflectionError, ReflectionLayoutConfig, ReflectionWorkspace,
};
use leaven_core::{ExternalRef, InfoRef};
use leaven_kernel::{Budget, BudgetSnapshot, Cost};
use leaven_workspace::{WorkspacePath, WorkspaceView};
use leaven_workspace_local::LocalWorkspaceFactory;
use serde_json::json;

#[test]
fn reflection_workspace_projects_runs_and_reads_back_current_tree() {
    block_on(async {
        let reflector = TextReflector;
        let runtime = FakeAgentRuntime::new(vec![
            FakeAgentAction::ReadFile {
                path: WorkspacePath::new("MANIFEST.json").unwrap(),
            },
            FakeAgentAction::ReadFile {
                path: WorkspacePath::new("CLAUDE.md").unwrap(),
            },
            FakeAgentAction::ReadFile {
                path: WorkspacePath::new("cases/case-000000.json").unwrap(),
            },
            FakeAgentAction::ReadFile {
                path: WorkspacePath::new("cross_case/source_refs.json").unwrap(),
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
        let case = json!({
            "input": "input",
            "feedback": "feedback"
        });
        let source_ref = InfoRef::External(ExternalRef {
            kind: "trace".to_owned(),
            id: "trace-1".to_owned(),
        });

        let outcome = ReflectionWorkspace::new(ReflectionLayoutConfig::default())
            .run(
                &reflector,
                &"seed",
                &[case],
                &[source_ref],
                &factory,
                &runtime,
                &budget,
            )
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

#[test]
fn reflection_workspace_reports_runtime_cost_and_declares_written_readonly_files() {
    block_on(async {
        let reflector = TextReflector;
        let runtime = FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
            path: WorkspacePath::new("target/current/result.txt").unwrap(),
            bytes: b"improved".to_vec(),
        }])
        .with_cost(Cost::llm_calls(3));
        let factory = LocalWorkspaceFactory::temp();
        let budget = BudgetSnapshot {
            limit: Budget::unlimited(),
            ..BudgetSnapshot::default()
        };

        let layout = ReflectionLayoutConfig::default();
        assert!(
            layout
                .readonly_roots
                .iter()
                .any(|path| path.as_str() == "CLAUDE.md")
        );

        let outcome = ReflectionWorkspace::new(layout)
            .run(
                &reflector,
                &"seed",
                &[] as &[serde_json::Value],
                &[],
                &factory,
                &runtime,
                &budget,
            )
            .await
            .unwrap();

        assert_eq!(outcome.cost, Cost::llm_calls(3));
    });
}

#[test]
fn reflection_workspace_rejects_non_successful_agent_session() {
    block_on(async {
        let reflector = TextReflector;
        let runtime = FakeAgentRuntime::new(vec![
            FakeAgentAction::WriteFile {
                path: WorkspacePath::new("target/current/result.txt").unwrap(),
                bytes: b"improved".to_vec(),
            },
            FakeAgentAction::Status(AgentStatus::Failed {
                reason: "agent stopped before finishing".to_owned(),
            }),
        ]);
        let factory = LocalWorkspaceFactory::temp();
        let budget = BudgetSnapshot {
            limit: Budget::unlimited(),
            ..BudgetSnapshot::default()
        };

        let error = ReflectionWorkspace::new(ReflectionLayoutConfig::default())
            .run(
                &reflector,
                &"seed",
                &[] as &[serde_json::Value],
                &[],
                &factory,
                &runtime,
                &budget,
            )
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ReflectionError::NonSucceededSession {
                status: AgentStatus::Failed { .. }
            }
        ));
    });
}

#[test]
fn reflection_workspace_rejects_agent_edits_outside_mutable_root() {
    block_on(async {
        let reflector = TextReflector;
        let runtime = FakeAgentRuntime::new(vec![
            FakeAgentAction::WriteFile {
                path: WorkspacePath::new("TASK.md").unwrap(),
                bytes: b"tampered".to_vec(),
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

        let error = ReflectionWorkspace::new(ReflectionLayoutConfig::default())
            .run(
                &reflector,
                &"seed",
                &[] as &[serde_json::Value],
                &[],
                &factory,
                &runtime,
                &budget,
            )
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ReflectionError::ProtectedWorkspaceModified { path } if path.as_str() == "TASK.md"
        ));
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
