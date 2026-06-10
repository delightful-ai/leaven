use serde_json::{Value, json};

/// Locked V1 Leaven extension method.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LockedMethod {
    /// Host-to-worker stage dispatch.
    StageRun,
    /// Graph read query.
    GraphQuery,
    /// Full case record read.
    CaseLoad,
    /// Case input read.
    CaseInput,
    /// Case target read.
    CaseTarget,
    /// Case metadata read.
    CaseMetadata,
    /// Workspace materialization call.
    WorkspaceMaterialize,
    /// Workspace snapshot read.
    WorkspaceSnapshot,
    /// Workspace listing read.
    WorkspaceList,
    /// Workspace file read.
    WorkspaceReadFile,
    /// Workspace stat read.
    WorkspaceStat,
    /// Workspace digest read.
    WorkspaceDigest,
    /// Workspace git log read.
    WorkspaceGitLog,
    /// Workspace git diff read.
    WorkspaceGitDiff,
    /// Workspace git status read.
    WorkspaceGitStatus,
    /// Workspace artifact capture read.
    WorkspaceCaptureArtifacts,
    /// Workspace release call.
    WorkspaceRelease,
    /// LM completion call.
    LmComplete,
    /// Agent run call.
    AgentRun,
    /// Sandbox execution call.
    SandboxExec,
    /// Proposal batch submission write.
    ProposalSubmitBatch,
    /// Proposal batch apply write.
    ProposalApply,
    /// Assessment submission write.
    AssessmentSubmit,
    /// Evaluation request write.
    EvaluationRequest,
    /// Run event emission write.
    EventEmit,
    /// Client-to-host optimization dispatch.
    OptimizeRun,
}

impl LockedMethod {
    /// Locked methods in canonical order: the 25 worker-profile methods followed
    /// by the one client-to-host dispatch method.
    pub const ALL: [Self; 26] = [
        Self::StageRun,
        Self::GraphQuery,
        Self::CaseLoad,
        Self::CaseInput,
        Self::CaseTarget,
        Self::CaseMetadata,
        Self::WorkspaceMaterialize,
        Self::WorkspaceSnapshot,
        Self::WorkspaceList,
        Self::WorkspaceReadFile,
        Self::WorkspaceStat,
        Self::WorkspaceDigest,
        Self::WorkspaceGitLog,
        Self::WorkspaceGitDiff,
        Self::WorkspaceGitStatus,
        Self::WorkspaceCaptureArtifacts,
        Self::WorkspaceRelease,
        Self::LmComplete,
        Self::AgentRun,
        Self::SandboxExec,
        Self::ProposalSubmitBatch,
        Self::ProposalApply,
        Self::AssessmentSubmit,
        Self::EvaluationRequest,
        Self::EventEmit,
        Self::OptimizeRun,
    ];

    /// Worker-profile methods in canonical order.
    ///
    /// These are the host<->worker methods the Leaven worker profile advertises:
    /// the 25-method worker callback surface plus the one `leaven/stage.run`
    /// dispatch. `OptimizeRun` is deliberately excluded because it is a
    /// client-to-host dispatch, not a worker callback or a host->worker stage
    /// dispatch.
    pub const WORKER_PROFILE: [Self; 25] = [
        Self::StageRun,
        Self::GraphQuery,
        Self::CaseLoad,
        Self::CaseInput,
        Self::CaseTarget,
        Self::CaseMetadata,
        Self::WorkspaceMaterialize,
        Self::WorkspaceSnapshot,
        Self::WorkspaceList,
        Self::WorkspaceReadFile,
        Self::WorkspaceStat,
        Self::WorkspaceDigest,
        Self::WorkspaceGitLog,
        Self::WorkspaceGitDiff,
        Self::WorkspaceGitStatus,
        Self::WorkspaceCaptureArtifacts,
        Self::WorkspaceRelease,
        Self::LmComplete,
        Self::AgentRun,
        Self::SandboxExec,
        Self::ProposalSubmitBatch,
        Self::ProposalApply,
        Self::AssessmentSubmit,
        Self::EvaluationRequest,
        Self::EventEmit,
    ];

    /// Whether this method is advertised by the host<->worker profile surface.
    ///
    /// The client-to-host `OptimizeRun` dispatch is locked but is not a worker
    /// callback or stage dispatch, so the worker profile never advertises it.
    pub const fn is_worker_profile_method(self) -> bool {
        !matches!(self, Self::OptimizeRun)
    }

    /// Parses a locked method name.
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "leaven/stage.run" => Self::StageRun,
            "leaven/graph.query" => Self::GraphQuery,
            "leaven/case.load" => Self::CaseLoad,
            "leaven/case.input" => Self::CaseInput,
            "leaven/case.target" => Self::CaseTarget,
            "leaven/case.metadata" => Self::CaseMetadata,
            "leaven/workspace.materialize" => Self::WorkspaceMaterialize,
            "leaven/workspace.snapshot" => Self::WorkspaceSnapshot,
            "leaven/workspace.list" => Self::WorkspaceList,
            "leaven/workspace.read_file" => Self::WorkspaceReadFile,
            "leaven/workspace.stat" => Self::WorkspaceStat,
            "leaven/workspace.digest" => Self::WorkspaceDigest,
            "leaven/workspace.git_log" => Self::WorkspaceGitLog,
            "leaven/workspace.git_diff" => Self::WorkspaceGitDiff,
            "leaven/workspace.git_status" => Self::WorkspaceGitStatus,
            "leaven/workspace.capture_artifacts" => Self::WorkspaceCaptureArtifacts,
            "leaven/workspace.release" => Self::WorkspaceRelease,
            "leaven/lm.complete" => Self::LmComplete,
            "leaven/agent.run" => Self::AgentRun,
            "leaven/sandbox.exec" => Self::SandboxExec,
            "leaven/proposal.submit_batch" => Self::ProposalSubmitBatch,
            "leaven/proposal.apply" => Self::ProposalApply,
            "leaven/assessment.submit" => Self::AssessmentSubmit,
            "leaven/evaluation.request" => Self::EvaluationRequest,
            "leaven/event.emit" => Self::EventEmit,
            "leaven/optimize.run" => Self::OptimizeRun,
            _ => return None,
        })
    }

    /// Wire method name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StageRun => "leaven/stage.run",
            Self::GraphQuery => "leaven/graph.query",
            Self::CaseLoad => "leaven/case.load",
            Self::CaseInput => "leaven/case.input",
            Self::CaseTarget => "leaven/case.target",
            Self::CaseMetadata => "leaven/case.metadata",
            Self::WorkspaceMaterialize => "leaven/workspace.materialize",
            Self::WorkspaceSnapshot => "leaven/workspace.snapshot",
            Self::WorkspaceList => "leaven/workspace.list",
            Self::WorkspaceReadFile => "leaven/workspace.read_file",
            Self::WorkspaceStat => "leaven/workspace.stat",
            Self::WorkspaceDigest => "leaven/workspace.digest",
            Self::WorkspaceGitLog => "leaven/workspace.git_log",
            Self::WorkspaceGitDiff => "leaven/workspace.git_diff",
            Self::WorkspaceGitStatus => "leaven/workspace.git_status",
            Self::WorkspaceCaptureArtifacts => "leaven/workspace.capture_artifacts",
            Self::WorkspaceRelease => "leaven/workspace.release",
            Self::LmComplete => "leaven/lm.complete",
            Self::AgentRun => "leaven/agent.run",
            Self::SandboxExec => "leaven/sandbox.exec",
            Self::ProposalSubmitBatch => "leaven/proposal.submit_batch",
            Self::ProposalApply => "leaven/proposal.apply",
            Self::AssessmentSubmit => "leaven/assessment.submit",
            Self::EvaluationRequest => "leaven/evaluation.request",
            Self::EventEmit => "leaven/event.emit",
            Self::OptimizeRun => "leaven/optimize.run",
        }
    }

    /// Required capability action.
    pub const fn required_action(self) -> MethodAction {
        match self {
            Self::StageRun => MethodAction::StageRun,
            Self::GraphQuery => MethodAction::GraphQuery,
            Self::CaseLoad | Self::CaseInput | Self::CaseTarget | Self::CaseMetadata => {
                MethodAction::CaseRead
            }
            Self::WorkspaceMaterialize => MethodAction::WorkspaceMaterialize,
            Self::WorkspaceSnapshot
            | Self::WorkspaceList
            | Self::WorkspaceReadFile
            | Self::WorkspaceStat
            | Self::WorkspaceDigest
            | Self::WorkspaceGitLog
            | Self::WorkspaceGitDiff
            | Self::WorkspaceGitStatus
            | Self::WorkspaceCaptureArtifacts => MethodAction::WorkspaceRead,
            Self::WorkspaceRelease => MethodAction::WorkspaceRelease,
            Self::LmComplete => MethodAction::LmComplete,
            Self::AgentRun => MethodAction::AgentRun,
            Self::SandboxExec => MethodAction::SandboxExec,
            Self::ProposalSubmitBatch => MethodAction::ProposalSubmitBatch,
            Self::ProposalApply => MethodAction::ProposalApplyBatch,
            Self::AssessmentSubmit => MethodAction::AssessmentSubmit,
            Self::EvaluationRequest => MethodAction::EvaluationRequest,
            Self::EventEmit => MethodAction::EventEmit,
            Self::OptimizeRun => MethodAction::OptimizeRun,
        }
    }

    /// Params schema bound to this method.
    pub const fn params_schema(self) -> MethodSchema {
        match self {
            Self::StageRun => MethodSchema::StageRun,
            Self::OptimizeRun => MethodSchema::OptimizeRun,
            _ => MethodSchema::PlanIr,
        }
    }

    /// Result schema bound to this method.
    pub const fn result_schema(self) -> MethodSchema {
        match self {
            Self::StageRun => MethodSchema::StageRun,
            Self::OptimizeRun => MethodSchema::OptimizeRun,
            _ => MethodSchema::PlanResult,
        }
    }

    /// Accepted primary result kinds for this method.
    pub const fn primary_kinds(self) -> &'static [MethodPrimaryKind] {
        match self {
            Self::StageRun => &[MethodPrimaryKind::StageRunTextOutput],
            Self::GraphQuery => &[MethodPrimaryKind::GraphSet],
            Self::CaseLoad | Self::CaseInput | Self::CaseTarget | Self::CaseMetadata => {
                &[MethodPrimaryKind::CaseRecord]
            }
            Self::WorkspaceMaterialize | Self::WorkspaceRelease => {
                &[MethodPrimaryKind::WorkspaceHandle]
            }
            Self::WorkspaceSnapshot | Self::WorkspaceDigest => {
                &[MethodPrimaryKind::WorkspaceSnapshot]
            }
            Self::WorkspaceReadFile => &[MethodPrimaryKind::WorkspaceFile],
            Self::WorkspaceList | Self::WorkspaceStat | Self::WorkspaceCaptureArtifacts => {
                &[MethodPrimaryKind::WorkspaceListing]
            }
            Self::WorkspaceGitLog | Self::WorkspaceGitDiff | Self::WorkspaceGitStatus => {
                &[MethodPrimaryKind::WorkspaceDiff]
            }
            Self::LmComplete => &[MethodPrimaryKind::LmResponse],
            Self::AgentRun => &[MethodPrimaryKind::AgentSession],
            Self::SandboxExec => &[MethodPrimaryKind::SandboxExec],
            Self::ProposalSubmitBatch => &[MethodPrimaryKind::ProposalBatchReceipt],
            Self::ProposalApply => &[MethodPrimaryKind::ApplyReceipt],
            Self::AssessmentSubmit => &[MethodPrimaryKind::AssessmentBatchReceipt],
            Self::EvaluationRequest => &[MethodPrimaryKind::EvaluationRequestReceipt],
            Self::EventEmit => &[MethodPrimaryKind::EmitRunEvent],
            Self::OptimizeRun => &[MethodPrimaryKind::OptimizedResult],
        }
    }

    /// Receipt expectation for this method.
    pub const fn receipt_expectation(self) -> MethodReceiptExpectation {
        match self {
            Self::StageRun => MethodReceiptExpectation::StageRun,
            Self::GraphQuery
            | Self::CaseLoad
            | Self::CaseInput
            | Self::CaseTarget
            | Self::CaseMetadata
            | Self::WorkspaceSnapshot
            | Self::WorkspaceList
            | Self::WorkspaceReadFile
            | Self::WorkspaceStat
            | Self::WorkspaceDigest
            | Self::WorkspaceGitLog
            | Self::WorkspaceGitDiff
            | Self::WorkspaceGitStatus
            | Self::WorkspaceCaptureArtifacts => MethodReceiptExpectation::Query,
            Self::WorkspaceMaterialize => MethodReceiptExpectation::Call("workspace_materialize"),
            Self::WorkspaceRelease => MethodReceiptExpectation::Call("workspace_release"),
            Self::LmComplete => MethodReceiptExpectation::Call("lm_complete"),
            Self::AgentRun => MethodReceiptExpectation::Call("agent_run"),
            Self::SandboxExec => MethodReceiptExpectation::Call("sandbox_exec"),
            Self::ProposalSubmitBatch => MethodReceiptExpectation::Write("submit_proposal_batch"),
            Self::ProposalApply => MethodReceiptExpectation::Write("apply_proposal_batch"),
            Self::AssessmentSubmit => MethodReceiptExpectation::Write("submit_assessments"),
            Self::EvaluationRequest => MethodReceiptExpectation::Write("request_evaluation"),
            Self::EventEmit => MethodReceiptExpectation::Write("emit_run_event"),
            Self::OptimizeRun => MethodReceiptExpectation::OptimizeRun,
        }
    }
}

/// Capability action required by a locked method.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodAction {
    /// `stage.run`
    StageRun,
    /// `graph.query`
    GraphQuery,
    /// `case.read`
    CaseRead,
    /// `workspace.materialize`
    WorkspaceMaterialize,
    /// `workspace.read`
    WorkspaceRead,
    /// `workspace.release`
    WorkspaceRelease,
    /// `lm.complete`
    LmComplete,
    /// `agent.run`
    AgentRun,
    /// `sandbox.exec`
    SandboxExec,
    /// `proposal.submit_batch`
    ProposalSubmitBatch,
    /// `proposal.apply_batch`
    ProposalApplyBatch,
    /// `assessment.submit`
    AssessmentSubmit,
    /// `evaluation.request`
    EvaluationRequest,
    /// `event.emit`
    EventEmit,
    /// `optimize.run`
    OptimizeRun,
}

impl MethodAction {
    /// Wire action spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StageRun => "stage.run",
            Self::GraphQuery => "graph.query",
            Self::CaseRead => "case.read",
            Self::WorkspaceMaterialize => "workspace.materialize",
            Self::WorkspaceRead => "workspace.read",
            Self::WorkspaceRelease => "workspace.release",
            Self::LmComplete => "lm.complete",
            Self::AgentRun => "agent.run",
            Self::SandboxExec => "sandbox.exec",
            Self::ProposalSubmitBatch => "proposal.submit_batch",
            Self::ProposalApplyBatch => "proposal.apply_batch",
            Self::AssessmentSubmit => "assessment.submit",
            Self::EvaluationRequest => "evaluation.request",
            Self::EventEmit => "event.emit",
            Self::OptimizeRun => "optimize.run",
        }
    }
}

/// The locked V1 worker-profile extension-method rows, in canonical order.
///
/// Each row carries the locked `params_schema`/`result_schema` binding, the
/// `required_action` capability path, and `produces_receipt`, exactly as the
/// profile validator demands. This is the single source the canonical locked
/// profile document is assembled from, so the engine client, the bridge, and the
/// conformance tests stop re-encoding the 25-method worker table by hand. The
/// client-to-host `leaven/optimize.run` dispatch is a locked method but is not a
/// worker-profile row, so it is excluded here.
pub(super) fn locked_extension_method_rows() -> Vec<Value> {
    LockedMethod::WORKER_PROFILE
        .into_iter()
        .map(|method| {
            json!({
                "method": method.as_str(),
                "params_schema": method.params_schema().schema_file(),
                "result_schema": method.result_schema().schema_file(),
                "required_action": method.required_action().as_str(),
                "produces_receipt": true
            })
        })
        .collect()
}

/// Schema bound to a Leaven ACP extension method's params or result.
///
/// The 25 worker callbacks bind the Plan IR schemas, the one host->worker
/// stage-dispatch method binds the dedicated stage-run schema, and the one
/// client->host optimization dispatch binds the dedicated optimize-run schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodSchema {
    /// Locked Plan IR request schema (`leaven.plan.v1.schema.json`).
    PlanIr,
    /// Locked Plan Result schema (`leaven.plan_result.v1.schema.json`).
    PlanResult,
    /// Dedicated stage-run schema (`leaven.stage_run.v1.schema.json`).
    StageRun,
    /// Dedicated optimize-run schema (`leaven.optimize_run.v1.schema.json`).
    OptimizeRun,
}

impl MethodSchema {
    /// JSON Schema file bound to this schema family.
    pub const fn schema_file(self) -> &'static str {
        match self {
            Self::PlanIr => "leaven.plan.v1.schema.json",
            Self::PlanResult => "leaven.plan_result.v1.schema.json",
            Self::StageRun => "leaven.stage_run.v1.schema.json",
            Self::OptimizeRun => "leaven.optimize_run.v1.schema.json",
        }
    }
}

/// Primary result kind bound to a locked method.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodPrimaryKind {
    /// Stage run text output.
    StageRunTextOutput,
    /// `graph_set`
    GraphSet,
    /// `case_record`
    CaseRecord,
    /// `workspace_handle`
    WorkspaceHandle,
    /// `workspace_snapshot`
    WorkspaceSnapshot,
    /// `workspace_file`
    WorkspaceFile,
    /// `workspace_listing`
    WorkspaceListing,
    /// `workspace_diff`
    WorkspaceDiff,
    /// `lm_response`
    LmResponse,
    /// `agent_session`
    AgentSession,
    /// `sandbox_exec`
    SandboxExec,
    /// `proposal_batch_receipt`
    ProposalBatchReceipt,
    /// `apply_receipt`
    ApplyReceipt,
    /// `assessment_batch_receipt`
    AssessmentBatchReceipt,
    /// `evaluation_request_receipt`
    EvaluationRequestReceipt,
    /// `emit_run_event`
    EmitRunEvent,
    /// `optimized_result`
    OptimizedResult,
}

impl MethodPrimaryKind {
    /// Wire primary-kind spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StageRunTextOutput => "stage_run_text_output",
            Self::GraphSet => "graph_set",
            Self::CaseRecord => "case_record",
            Self::WorkspaceHandle => "workspace_handle",
            Self::WorkspaceSnapshot => "workspace_snapshot",
            Self::WorkspaceFile => "workspace_file",
            Self::WorkspaceListing => "workspace_listing",
            Self::WorkspaceDiff => "workspace_diff",
            Self::LmResponse => "lm_response",
            Self::AgentSession => "agent_session",
            Self::SandboxExec => "sandbox_exec",
            Self::ProposalBatchReceipt => "proposal_batch_receipt",
            Self::ApplyReceipt => "apply_receipt",
            Self::AssessmentBatchReceipt => "assessment_batch_receipt",
            Self::EvaluationRequestReceipt => "evaluation_request_receipt",
            Self::EmitRunEvent => "emit_run_event",
            Self::OptimizedResult => "optimized_result",
        }
    }

    /// Parses a locked primary-kind spelling.
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "stage_run_text_output" => Self::StageRunTextOutput,
            "graph_set" => Self::GraphSet,
            "case_record" => Self::CaseRecord,
            "workspace_handle" => Self::WorkspaceHandle,
            "workspace_snapshot" => Self::WorkspaceSnapshot,
            "workspace_file" => Self::WorkspaceFile,
            "workspace_listing" => Self::WorkspaceListing,
            "workspace_diff" => Self::WorkspaceDiff,
            "lm_response" => Self::LmResponse,
            "agent_session" => Self::AgentSession,
            "sandbox_exec" => Self::SandboxExec,
            "proposal_batch_receipt" => Self::ProposalBatchReceipt,
            "apply_receipt" => Self::ApplyReceipt,
            "assessment_batch_receipt" => Self::AssessmentBatchReceipt,
            "evaluation_request_receipt" => Self::EvaluationRequestReceipt,
            "emit_run_event" => Self::EmitRunEvent,
            "optimized_result" => Self::OptimizedResult,
            _ => return None,
        })
    }
}

/// Receipt family expected from one locked method.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodReceiptExpectation {
    /// Stage-run result receipt.
    StageRun,
    /// Optimize-run result receipt.
    OptimizeRun,
    /// Query receipt.
    Query,
    /// Call receipt with expected call kind.
    Call(&'static str),
    /// Write receipt with expected write kind.
    Write(&'static str),
}
