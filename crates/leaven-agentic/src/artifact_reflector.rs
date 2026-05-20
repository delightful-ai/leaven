use std::collections::BTreeMap;
use std::future::Future;

use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, AgentRuntimeError,
    AgentSession, AgentStatus, OutputContract,
};
use leaven_core::InfoRef;
use leaven_kernel::{AgentSessionId, BudgetSnapshot, Cost};
use leaven_workspace::{
    FactoryError, WorkspaceConfig, WorkspaceError, WorkspaceFactory, WorkspacePath, WorkspaceView,
    fingerprint_file,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Artifact-specific seam for generic agentic reflection.
pub trait ArtifactReflector: Send + Sync {
    type Input: Send + Sync;
    type Change: Send + Sync;
    type Error: std::error::Error + Send + Sync + 'static;

    /// Stable reflection identity written into `MANIFEST.json`.
    fn reflection_id(&self) -> &'static str;

    /// Project the typed artifact input into `target/current`.
    fn project<'a>(
        &'a self,
        input: &'a Self::Input,
        view: &'a mut WorkspaceView<'_>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'a;

    /// Read agent edits in `target/current` back into a typed change.
    fn read_back<'a>(
        &'a self,
        input: &'a Self::Input,
        view: &'a WorkspaceView<'_>,
        session: &'a AgentSession,
    ) -> impl Future<Output = Result<ReadbackResult<Self::Change>, Self::Error>> + Send + 'a;
}

/// Result of lowering an edited reflection workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadbackResult<T> {
    Valid(T),
    Empty,
    Invalid {
        diagnostics: Vec<ReadbackDiagnostic>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReadbackDiagnostic {
    pub path: Option<WorkspacePath>,
    pub message: String,
}

/// Generic runner for one agentic reflection transaction.
#[derive(Clone, Debug)]
pub struct ReflectionWorkspace {
    layout: ReflectionLayoutConfig,
    workspace_config: WorkspaceConfig,
}

impl ReflectionWorkspace {
    #[must_use]
    pub fn new(layout: ReflectionLayoutConfig) -> Self {
        Self {
            layout,
            workspace_config: WorkspaceConfig::default(),
        }
    }

    #[must_use]
    pub fn with_workspace_config(mut self, workspace_config: WorkspaceConfig) -> Self {
        self.workspace_config = workspace_config;
        self
    }

    pub async fn run<R, Case, Factory, Runtime>(
        &self,
        reflector: &R,
        input: &R::Input,
        cases: &[Case],
        source_refs: &[InfoRef],
        factory: &Factory,
        runtime: &Runtime,
        budget: &BudgetSnapshot,
    ) -> Result<ReflectionRunOutcome<R::Change>, ReflectionError<R::Error>>
    where
        R: ArtifactReflector,
        Case: Serialize + Sync,
        Factory: WorkspaceFactory,
        Runtime: AgentRuntime,
    {
        let mut workspace = factory
            .allocate(self.workspace_config.clone())
            .await
            .map_err(ReflectionError::Allocate)?;
        let stage_result = async {
            let mut view = workspace.view();
            write_workspace_contract::<R::Error, Case>(
                &mut view,
                reflector.reflection_id(),
                &self.layout,
                cases,
                source_refs,
            )?;
            {
                let mut current = view.subdir(self.layout.mutable_root.clone())?;
                reflector
                    .project(input, &mut current)
                    .await
                    .map_err(ReflectionError::Project)?;
            }
            let protected_before = protected_workspace_snapshot(&view, &self.layout.mutable_root)?;

            let session = runtime
                .run_session(
                    &mut view,
                    AgentRunRequest {
                        instructions: AgentInstructions::task(format!(
                            "Read TASK.md and edit {} in place.",
                            self.layout.mutable_root.as_str()
                        )),
                        cwd: self.layout.mutable_root.clone(),
                        output_contract: OutputContract::WorkspaceDiff {
                            roots: vec![self.layout.mutable_root.clone()],
                        },
                        env: Default::default(),
                        tool_policy: Default::default(),
                        limits: Default::default(),
                    },
                    AgentRunContext::new(AgentSessionId::new(), budget),
                )
                .await
                .map_err(ReflectionError::Runtime)?;
            let session_cost = session.cost;
            let session = session.value;
            if session.status != AgentStatus::Succeeded {
                return Err(ReflectionError::NonSucceededSession {
                    status: session.status,
                });
            }
            ensure_protected_workspace_unchanged(
                &view,
                &self.layout.mutable_root,
                &protected_before,
            )?;

            let readback = {
                let current = view.subdir(self.layout.mutable_root.clone())?;
                reflector
                    .read_back(input, &current, &session)
                    .await
                    .map_err(ReflectionError::Readback)?
            };
            let attachments = session_attachments(&session)?;
            Ok(ReflectionRunOutcome {
                readback,
                session_attachments: attachments,
                cost: session_cost,
            })
        }
        .await;
        let cleanup_result = workspace.cleanup().await;

        match (stage_result, cleanup_result) {
            (Ok(outcome), Ok(())) => Ok(outcome),
            (Ok(_), Err(cleanup)) => Err(ReflectionError::Workspace(cleanup)),
            (Err(stage), Ok(())) => Err(stage),
            (Err(stage), Err(cleanup)) => Err(ReflectionError::StageAndCleanup {
                stage: Box::new(stage),
                cleanup,
            }),
        }
    }
}

impl Default for ReflectionWorkspace {
    fn default() -> Self {
        Self::new(ReflectionLayoutConfig::default())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReflectionLayoutConfig {
    pub mutable_root: WorkspacePath,
    pub readonly_roots: Vec<WorkspacePath>,
    pub inline_text_threshold: usize,
    pub full_trace_top_k: usize,
}

impl Default for ReflectionLayoutConfig {
    fn default() -> Self {
        Self {
            mutable_root: WorkspacePath::new("target/current").expect("constant path is valid"),
            readonly_roots: vec![
                WorkspacePath::new("cases").expect("constant path is valid"),
                WorkspacePath::new("cross_case").expect("constant path is valid"),
                WorkspacePath::new("MANIFEST.json").expect("constant path is valid"),
                WorkspacePath::new("TASK.md").expect("constant path is valid"),
                WorkspacePath::new("AGENTS.md").expect("constant path is valid"),
                WorkspacePath::new("CLAUDE.md").expect("constant path is valid"),
            ],
            inline_text_threshold: 512,
            full_trace_top_k: 5,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReflectionRunOutcome<C> {
    pub readback: ReadbackResult<C>,
    pub session_attachments: Vec<ReflectionSessionAttachment>,
    pub cost: Cost,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReflectionSessionAttachment {
    pub name: String,
    pub json: serde_json::Value,
    pub media_type: Option<String>,
}

#[derive(Debug, Error)]
pub enum ReflectionError<E: std::error::Error + Send + Sync + 'static> {
    #[error(transparent)]
    Allocate(#[from] FactoryError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Runtime(#[from] AgentRuntimeError),
    #[error("artifact projection failed")]
    Project(#[source] E),
    #[error("artifact readback failed")]
    Readback(#[source] E),
    #[error("manifest serialization failed")]
    Manifest(#[source] serde_json::Error),
    #[error("agent session finished without success: {status:?}")]
    NonSucceededSession { status: AgentStatus },
    #[error("agent session modified protected workspace path `{path}`")]
    ProtectedWorkspaceModified { path: WorkspacePath },
    #[error("proposal failed")]
    Proposal(#[source] leaven_engine::ProposalError),
    #[error("reflection stage failed and cleanup failed")]
    StageAndCleanup {
        #[source]
        stage: Box<Self>,
        cleanup: WorkspaceError,
    },
    #[error("cancelled")]
    Cancelled,
    #[error("budget exhausted")]
    BudgetExhausted,
}

impl<E> From<leaven_engine::ProposalError> for ReflectionError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn from(error: leaven_engine::ProposalError) -> Self {
        Self::Proposal(error)
    }
}

type ProtectedWorkspaceSnapshot =
    BTreeMap<WorkspacePath, leaven_workspace::WorkspaceFileFingerprint>;

fn protected_workspace_snapshot<E>(
    view: &WorkspaceView<'_>,
    mutable_root: &WorkspacePath,
) -> Result<ProtectedWorkspaceSnapshot, ReflectionError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    let mut snapshot = BTreeMap::new();
    for path in view.list_files(&WorkspacePath::root())? {
        if path_is_under(&path, mutable_root) {
            continue;
        }
        snapshot.insert(path.clone(), fingerprint_file(view, &path)?);
    }
    Ok(snapshot)
}

fn ensure_protected_workspace_unchanged<E>(
    view: &WorkspaceView<'_>,
    mutable_root: &WorkspacePath,
    before: &ProtectedWorkspaceSnapshot,
) -> Result<(), ReflectionError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    let after = protected_workspace_snapshot(view, mutable_root)?;
    if before == &after {
        return Ok(());
    }
    let changed = before
        .keys()
        .chain(after.keys())
        .find(|path| before.get(*path) != after.get(*path))
        .cloned()
        .unwrap_or_else(WorkspacePath::root);
    Err(ReflectionError::ProtectedWorkspaceModified { path: changed })
}

fn path_is_under(path: &WorkspacePath, root: &WorkspacePath) -> bool {
    if root.as_str().is_empty() {
        return true;
    }
    path.as_str() == root.as_str()
        || path
            .as_str()
            .strip_prefix(root.as_str())
            .is_some_and(|suffix| suffix.starts_with('/'))
}

#[derive(Serialize)]
struct ReflectionManifest<'a> {
    kind: &'static str,
    signature_id: &'a str,
    mutable_root: &'a str,
    readonly_roots: Vec<&'a str>,
    case_count: usize,
    source_ref_count: usize,
    inline_text_threshold: usize,
    full_trace_top_k: usize,
}

fn write_workspace_contract<E, Case>(
    view: &mut WorkspaceView<'_>,
    signature_id: &str,
    layout: &ReflectionLayoutConfig,
    cases: &[Case],
    source_refs: &[InfoRef],
) -> Result<(), ReflectionError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
    Case: Serialize,
{
    let manifest = ReflectionManifest {
        kind: "leaven.agentic_reflection_workspace.v1",
        signature_id,
        mutable_root: layout.mutable_root.as_str(),
        readonly_roots: layout
            .readonly_roots
            .iter()
            .map(WorkspacePath::as_str)
            .collect(),
        case_count: cases.len(),
        source_ref_count: source_refs.len(),
        inline_text_threshold: layout.inline_text_threshold,
        full_trace_top_k: layout.full_trace_top_k,
    };
    let manifest = serde_json::to_vec_pretty(&manifest).map_err(ReflectionError::Manifest)?;
    view.write_file(
        &WorkspacePath::new("MANIFEST.json").expect("constant path is valid"),
        &manifest,
    )?;
    for (index, case) in cases.iter().enumerate() {
        let path = WorkspacePath::new(format!("cases/case-{index:06}.json"))
            .expect("case path is generated from numeric index");
        let bytes = serde_json::to_vec_pretty(case).map_err(ReflectionError::Manifest)?;
        view.write_file(&path, &bytes)?;
    }
    let source_refs = serde_json::to_vec_pretty(source_refs).map_err(ReflectionError::Manifest)?;
    view.write_file(
        &WorkspacePath::new("cross_case/source_refs.json").expect("constant path is valid"),
        &source_refs,
    )?;
    view.write_file(
        &WorkspacePath::new("TASK.md").expect("constant path is valid"),
        format!(
            "# Reflection task\n\nYou are improving the artifact in `{}`.\n\nRead `MANIFEST.json`, inspect the evidence files when present, and edit only `{}`.\n",
            layout.mutable_root.as_str(),
            layout.mutable_root.as_str()
        )
        .as_bytes(),
    )?;
    view.write_file(
        &WorkspacePath::new("AGENTS.md").expect("constant path is valid"),
        format!(
            "# Reflection workspace rules\n\nOnly `{}` is mutable. Evidence under `cases` and `cross_case` is read-only.\n",
            layout.mutable_root.as_str()
        )
        .as_bytes(),
    )?;
    view.write_file(
        &WorkspacePath::new("CLAUDE.md").expect("constant path is valid"),
        format!(
            "# Reflection workspace rules\n\nOnly `{}` is mutable. Evidence under `cases` and `cross_case` is read-only.\n",
            layout.mutable_root.as_str()
        )
        .as_bytes(),
    )?;
    Ok(())
}

fn session_attachments<E>(
    session: &AgentSession,
) -> Result<Vec<ReflectionSessionAttachment>, ReflectionError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    let mut attachments = vec![ReflectionSessionAttachment {
        name: "session/main".to_owned(),
        json: serde_json::to_value(&session.transcript).map_err(ReflectionError::Manifest)?,
        media_type: Some("application/json".to_owned()),
    }];
    if !session.commands.is_empty() {
        attachments.push(ReflectionSessionAttachment {
            name: "session/commands".to_owned(),
            json: serde_json::to_value(&session.commands).map_err(ReflectionError::Manifest)?,
            media_type: Some("application/json".to_owned()),
        });
    }
    if !session.raw_provider_events.is_empty() {
        attachments.push(ReflectionSessionAttachment {
            name: "session/raw_provider_events".to_owned(),
            json: serde_json::to_value(&session.raw_provider_events)
                .map_err(ReflectionError::Manifest)?,
            media_type: Some("application/json".to_owned()),
        });
    }
    Ok(attachments)
}
