"""Plan IR request construction for private public-seam clients."""

from collections.abc import Sequence
from dataclasses import dataclass
from typing import Literal, Protocol

from msgspec import UNSET, UnsetType, convert

from leaven._seam._wire import JsonObject
from leaven._seam._wire.calls import (
    AgentInstructions,
    AgentLimits,
    AgentRunCall,
    AgentToolPolicy,
    LmCompleteCall,
    LmMessage,
    LmOutputContract,
    LmOutputFinalMessage,
    LmOutputJsonSchema,
    LmSampling,
    OutputContract,
    OutputFiles,
    OutputFinalMessage,
    OutputJsonSchema,
    OutputWorkspaceDiff,
    SandboxExecCall,
    WorkspaceMaterializeCall,
)
from leaven._seam._wire.codec import RequestParams
from leaven._seam._wire.expressions import PlanExpressionCaseQuery
from leaven._seam._wire.payloads import (
    CommitPolicyGraphWritesAtomic,
    CommitPolicyNoGraphWrites,
    ConsistencyLatestAtStart,
    EvalModeExecute,
    FailureMode,
    PlanDocument,
    PlanOp,
    ProposeRequest,
    ReflectionResult,
    RunnerRequest,
)
from leaven._seam._wire.payloads import StageRunRequest as StageRunParams
from leaven._seam._wire.refs import WireJsonField, WireJsonSchemaObject
from leaven._seam._wire.writes import (
    ApplyProposalBatchWrite,
    ProposalWriteRecord,
    SubmitAssessmentRecord,
    SubmitAssessmentsWrite,
    SubmitProposalBatchWrite,
)

CaseField = Literal["input", "target", "metadata", "files", "setup", "sandbox", "split"]
SeamRequestMethod = Literal[
    "leaven/stage.run",
    "leaven/agent.run",
    "leaven/assessment.submit",
    "leaven/case.load",
    "leaven/case.input",
    "leaven/case.target",
    "leaven/case.metadata",
    "leaven/lm.complete",
    "leaven/proposal.apply",
    "leaven/proposal.submit_batch",
    "leaven/sandbox.exec",
    "leaven/workspace.capture_artifacts",
    "leaven/workspace.digest",
    "leaven/workspace.git_diff",
    "leaven/workspace.git_log",
    "leaven/workspace.git_status",
    "leaven/workspace.list",
    "leaven/workspace.materialize",
    "leaven/workspace.read_file",
    "leaven/workspace.release",
    "leaven/workspace.snapshot",
    "leaven/workspace.stat",
]


_SINGLE_CASE_METHODS: dict[tuple[CaseField, ...], tuple[SeamRequestMethod, str]] = {
    ("input",): ("leaven/case.input", "case_input"),
    ("target",): ("leaven/case.target", "case_target"),
    ("metadata",): ("leaven/case.metadata", "case_metadata"),
}


class SeamJsonRpcRequest(Protocol):
    """Typed request record that can lower itself to locked JSON-RPC params."""

    request_id: str

    @property
    def method(self) -> SeamRequestMethod:
        """Locked Leaven public-seam method this request targets."""
        ...

    def to_params(self) -> RequestParams:
        """Return the method-specific JSON-RPC params object."""
        ...


@dataclass(frozen=True)
class CaseLoadRequest:
    """A single public-seam case read Plan request."""

    request_id: str
    plan_id: str
    case_id: str
    include: Sequence[CaseField]
    run_id: str = "run_python_case_builder"

    @property
    def method(self) -> SeamRequestMethod:
        """Locked case read method selected by `include`."""
        return _case_route(self.include)[0]

    def to_params(self) -> PlanDocument:
        """Return the locked case read Plan params."""
        _, op_name = _case_route(self.include)
        return _plan_document(
            plan_id=self.plan_id,
            ops=[self._case_query(op_name)],
            return_names=[op_name],
            commit=CommitPolicyNoGraphWrites(),
        )

    def _case_query(self, op_name: str) -> PlanOp:
        return PlanOp(
            kind="let",
            name=op_name,
            expr=PlanExpressionCaseQuery(
                query={
                    "kind": "load",
                    "case": {
                        "kind": "case",
                        "run": self.run_id,
                        "id": self.case_id,
                    },
                    "include": list(self.include),
                    "projection_schema": "fp_schema_sha256_python_case_projection",
                }
            ),
        )


def _case_route(include: Sequence[CaseField]) -> tuple[SeamRequestMethod, str]:
    key = tuple(include)
    if key in _SINGLE_CASE_METHODS:
        return _SINGLE_CASE_METHODS[key]
    return ("leaven/case.load", "case_load")


@dataclass(frozen=True)
class AgentRunRequest:
    """A single public-seam `leaven/agent.run` Plan request."""

    request_id: str
    plan_id: str
    candidate: str
    workspace: str
    instructions: dict[str, str]
    idempotency_prefix: str
    runtime: str = "codex-cli"
    timeout_s: int = 180
    max_turns: int = 1
    max_usd_micro: int = 5_000_000
    output: JsonObject | None = None
    allowed_commands: Sequence[str] | None = None
    input_classes: Sequence[str] | None = None
    forbidden_input_classes: Sequence[str] | None = None

    @property
    def method(self) -> SeamRequestMethod:
        """Locked agent method."""
        return "leaven/agent.run"

    def to_params(self) -> PlanDocument:
        """Return the locked agent Plan params."""
        return _plan_document(
            plan_id=self.plan_id,
            ops=[self._workspace_call(), self._agent_call()],
            return_names=["workspace", "completion"],
            commit=CommitPolicyGraphWritesAtomic(on_stale="reject"),
        )

    def _workspace_call(self) -> PlanOp:
        return PlanOp(
            kind="call",
            name="workspace",
            idempotency_key=f"{self.idempotency_prefix}-workspace",
            call=WorkspaceMaterializeCall(
                candidate=self.candidate,
                surface="program",
                mode="copy_on_write",
                lifetime="manual_release",
            ),
        )

    def _agent_call(self) -> PlanOp:
        allowed_commands = (
            list(self.allowed_commands) if self.allowed_commands is not None else UNSET
        )
        return PlanOp(
            kind="call",
            name="completion",
            deps=["workspace"],
            idempotency_key=f"{self.idempotency_prefix}-agent",
            call=AgentRunCall(
                runtime=self.runtime,
                workspace=self.workspace,
                instructions=convert(self.instructions, type=AgentInstructions),
                tool_policy=AgentToolPolicy(
                    allow_shell=False,
                    allowed_commands=allowed_commands,
                ),
                output=_wire_output_contract(
                    self.output or {"kind": "final_message", "max_bytes": 512}
                ),
                limits=AgentLimits(
                    timeout_s=self.timeout_s,
                    max_turns=self.max_turns,
                    max_usd_micro=self.max_usd_micro,
                ),
                input_classes=list(self.input_classes or ["public"]),
                forbidden_input_classes=(
                    list(self.forbidden_input_classes)
                    if self.forbidden_input_classes is not None
                    else UNSET
                ),
            ),
        )


@dataclass(frozen=True)
class LmCompleteRequest:
    """A single public-seam `leaven/lm.complete` Plan request."""

    request_id: str
    plan_id: str
    idempotency_key: str
    messages: Sequence[JsonObject]
    model: str
    model_role: str | None = None
    temperature: float | None = None
    max_tokens: int | None = None
    stop: Sequence[str] | None = None
    output: JsonObject | None = None
    input_classes: Sequence[str] | None = None

    @property
    def method(self) -> SeamRequestMethod:
        """Locked LM method."""
        return "leaven/lm.complete"

    def to_params(self) -> PlanDocument:
        """Return the locked LM Plan params."""
        return _plan_document(
            plan_id=self.plan_id,
            ops=[self._lm_call()],
            return_names=["completion"],
            commit=CommitPolicyNoGraphWrites(),
        )

    def _lm_call(self) -> PlanOp:
        sampling = {}
        if self.temperature is not None:
            sampling["temperature"] = self.temperature
        if self.max_tokens is not None:
            sampling["max_output_tokens"] = self.max_tokens
        if self.stop is not None:
            sampling["stop"] = list(self.stop)
        return PlanOp(
            kind="call",
            name="completion",
            idempotency_key=self.idempotency_key,
            call=LmCompleteCall(
                purpose="python.sdk",
                model=self.model,
                model_role=self.model_role if self.model_role is not None else UNSET,
                messages=[convert(message, type=LmMessage) for message in self.messages],
                output=_wire_lm_output_contract(
                    self.output or {"kind": "final_message", "max_bytes": 512}
                ),
                sampling=convert(sampling, type=LmSampling) if sampling else UNSET,
                input_classes=list(self.input_classes or ["public"]),
            ),
        )


@dataclass(frozen=True)
class AssessmentSubmitRequest:
    """A single public-seam `leaven/assessment.submit` Plan request."""

    request_id: str
    plan_id: str
    idempotency_key: str
    evaluation_request_id: str
    assessments: Sequence[SubmitAssessmentRecord]

    @property
    def method(self) -> SeamRequestMethod:
        """Locked assessment submission method."""
        return "leaven/assessment.submit"

    def to_params(self) -> PlanDocument:
        """Return the locked assessment submission Plan params."""
        return _plan_document(
            plan_id=self.plan_id,
            ops=[self._assessment_write()],
            return_names=["assessment_batch"],
            commit=CommitPolicyGraphWritesAtomic(on_stale="reject"),
        )

    def _assessment_write(self) -> PlanOp:
        return PlanOp(
            kind="write",
            name="assessment_batch",
            idempotency_key=self.idempotency_key,
            write=SubmitAssessmentsWrite(
                evaluation_request_id=self.evaluation_request_id,
                assessments=list(self.assessments),
            ),
        )


@dataclass(frozen=True)
class SandboxExecRequest:
    """A single public-seam `leaven/sandbox.exec` Plan request."""

    request_id: str
    plan_id: str
    idempotency_key: str
    workspace: str
    argv: Sequence[str]
    timeout_s: int
    output: JsonObject
    env: dict[str, str] | None = None
    cwd: str | None = None
    stream_policy: Literal["buffer", "stream_updates", "blob_refs_only"] = "blob_refs_only"
    input_classes: Sequence[str] | None = None
    forbidden_input_classes: Sequence[str] | None = None

    @property
    def method(self) -> SeamRequestMethod:
        """Locked sandbox method."""
        return "leaven/sandbox.exec"

    def to_params(self) -> PlanDocument:
        """Return the locked sandbox Plan params."""
        return _plan_document(
            plan_id=self.plan_id,
            ops=[self._sandbox_call()],
            return_names=["sandbox_exec"],
            commit=CommitPolicyNoGraphWrites(),
        )

    def _sandbox_call(self) -> PlanOp:
        return PlanOp(
            kind="call",
            name="sandbox_exec",
            idempotency_key=self.idempotency_key,
            call=SandboxExecCall(
                workspace=self.workspace,
                argv=list(self.argv),
                timeout_s=self.timeout_s,
                output=_wire_output_contract(self.output),
                input_classes=list(self.input_classes or ["public"]),
                cwd=self.cwd if self.cwd is not None else UNSET,
                env=dict(self.env) if self.env is not None else UNSET,
                stream_policy=self.stream_policy,
                forbidden_input_classes=(
                    list(self.forbidden_input_classes)
                    if self.forbidden_input_classes is not None
                    else UNSET
                ),
            ),
        )


@dataclass(frozen=True)
class StageRunRequest:
    """A single public-seam `leaven/stage.run` runner dispatch request."""

    request_id: str
    run_id: str
    stage_call_id: str
    candidate: str
    case: str
    case_input: WireJsonField

    @property
    def method(self) -> SeamRequestMethod:
        """Locked stage-run method."""
        return "leaven/stage.run"

    def to_params(self) -> StageRunParams:
        """Return the locked runner stage dispatch params."""
        return StageRunParams(
            schema_version="leaven.stage_run.v1",
            message="stage_run_request",
            stage="runner",
            payload=RunnerRequest(
                schema_version="leaven.stage_payloads.v1",
                run=self.run_id,
                stage_call_id=self.stage_call_id,
                candidate=self.candidate,
                case=self.case,
                case_input=self.case_input,
                target_forbidden=True,
            ),
        )


@dataclass(frozen=True)
class StageRunProposeRequest:
    """A single public-seam `leaven/stage.run` proposer dispatch request."""

    request_id: str
    run_id: str
    stage_call_id: str
    base_revision: str
    parent: str
    surface_fingerprint: str
    change_schema: str
    capability_fingerprint: str
    query_policy_fingerprint: str
    reflection_summary: str

    @property
    def method(self) -> SeamRequestMethod:
        """Locked stage-run method."""
        return "leaven/stage.run"

    def to_params(self) -> StageRunParams:
        """Return the locked proposer stage dispatch params."""
        return StageRunParams(
            schema_version="leaven.stage_run.v1",
            message="stage_run_request",
            stage="proposer",
            payload=ProposeRequest(
                schema_version="leaven.stage_payloads.v1",
                run=self.run_id,
                stage_call_id=self.stage_call_id,
                base_revision=self.base_revision,
                parent=self.parent,
                surface_fingerprint=self.surface_fingerprint,
                reflection_result=self._reflection_result(),
                allowed_effects=["change"],
                allowed_change_schemas=[self.change_schema],
                source_refs=[self.parent],
                query_policy_fingerprint=self.query_policy_fingerprint,
                capability_fingerprint=self.capability_fingerprint,
            ),
        )

    def _reflection_result(self) -> ReflectionResult:
        return ReflectionResult(
            schema_version="leaven.stage_payloads.v1",
            summary=self.reflection_summary,
            failure_modes=[
                FailureMode(
                    label="seed_assessment_feedback",
                    description=self.reflection_summary,
                    source_refs=[self.parent],
                )
            ],
            surface_suggestions=[],
            negative_constraints=[],
            positive_constraints=[],
            source_refs=[self.parent],
            read_receipts=["qrec_python_seed_assessment"],
            data_classes=["optimizer.visible"],
            confidence=0.5,
        )


@dataclass(frozen=True)
class ProposalSubmitRequest:
    """A single public-seam `leaven/proposal.submit_batch` Plan request."""

    request_id: str
    plan_id: str
    idempotency_key: str
    proposals: Sequence[JsonObject]

    @property
    def method(self) -> SeamRequestMethod:
        """Locked proposal submission method."""
        return "leaven/proposal.submit_batch"

    def to_params(self) -> PlanDocument:
        """Return the locked proposal submission Plan params."""
        return _plan_document(
            plan_id=self.plan_id,
            ops=[self._submit_write()],
            return_names=["proposal_batch"],
            commit=CommitPolicyGraphWritesAtomic(on_stale="reject"),
        )

    def _submit_write(self) -> PlanOp:
        return PlanOp(
            kind="write",
            name="proposal_batch",
            idempotency_key=self.idempotency_key,
            write=SubmitProposalBatchWrite(
                semantics="sequence",
                proposals=[
                    convert(proposal, type=ProposalWriteRecord) for proposal in self.proposals
                ],
            ),
        )


@dataclass(frozen=True)
class ProposalApplyRequest:
    """A single public-seam `leaven/proposal.apply` Plan request."""

    request_id: str
    plan_id: str
    idempotency_key: str
    proposal_batch: str
    policy: Literal["apply_all", "apply_first_valid", "apply_by_optimizer_policy"] = (
        "apply_first_valid"
    )

    @property
    def method(self) -> SeamRequestMethod:
        """Locked proposal application method."""
        return "leaven/proposal.apply"

    def to_params(self) -> PlanDocument:
        """Return the locked proposal application Plan params."""
        return _plan_document(
            plan_id=self.plan_id,
            ops=[self._apply_write()],
            return_names=["apply"],
            commit=CommitPolicyGraphWritesAtomic(on_stale="reject"),
        )

    def _apply_write(self) -> PlanOp:
        return PlanOp(
            kind="write",
            name="apply",
            idempotency_key=self.idempotency_key,
            write=ApplyProposalBatchWrite(
                proposal_batch=self.proposal_batch,
                policy=self.policy,
            ),
        )


def _plan_document(
    *,
    plan_id: str,
    ops: list[PlanOp],
    return_names: list[str],
    commit: CommitPolicyNoGraphWrites | CommitPolicyGraphWritesAtomic,
) -> PlanDocument:
    return PlanDocument(
        schema_version="leaven.plan.v1",
        plan_id=plan_id,
        consistency=ConsistencyLatestAtStart(),
        mode=EvalModeExecute(),
        ops=ops,
        return_=return_names,
        commit=commit,
    )


def _wire_output_contract(value: JsonObject) -> OutputContract:
    kind = _string_field(value, "kind")
    if kind == "final_message":
        return OutputFinalMessage(max_bytes=_optional_int_field(value, "max_bytes"))
    if kind == "json_schema":
        return OutputJsonSchema(
            schema_fingerprint=_string_field(value, "schema_fingerprint"),
            schema=_optional_json_schema_field(value, "schema"),
        )
    if kind == "files":
        return OutputFiles(
            paths=_string_list_field(value, "paths"),
            max_bytes=_optional_int_field(value, "max_bytes"),
        )
    if kind == "workspace_diff":
        return OutputWorkspaceDiff(
            surface_fingerprint=_string_field(value, "surface_fingerprint"),
            max_bytes=_optional_int_field(value, "max_bytes"),
        )
    raise ValueError(f"unsupported output contract kind: {kind}")


def _wire_lm_output_contract(value: JsonObject) -> LmOutputContract:
    kind = _string_field(value, "kind")
    if kind == "final_message":
        return LmOutputFinalMessage(max_bytes=_optional_int_field(value, "max_bytes"))
    if kind == "json_schema":
        return LmOutputJsonSchema(
            schema_fingerprint=_string_field(value, "schema_fingerprint"),
            schema=_optional_json_schema_field(value, "schema"),
        )
    raise ValueError(f"unsupported LM output contract kind: {kind}")


def _string_field(value: JsonObject, field: str) -> str:
    item = value[field]
    if not isinstance(item, str):
        raise TypeError(f"{field} must be a string")
    return item


def _optional_int_field(value: JsonObject, field: str) -> int | UnsetType:
    if field not in value:
        return UNSET
    item = value[field]
    if not isinstance(item, int):
        raise TypeError(f"{field} must be an integer")
    return item


def _string_list_field(value: JsonObject, field: str) -> list[str]:
    item = value[field]
    if not isinstance(item, list) or not all(isinstance(member, str) for member in item):
        raise TypeError(f"{field} must be a list of strings")
    return [member for member in item if isinstance(member, str)]


def _optional_json_schema_field(value: JsonObject, field: str) -> WireJsonSchemaObject | UnsetType:
    if field not in value:
        return UNSET
    return convert(value[field], type=WireJsonSchemaObject)


__all__ = [
    "AgentRunRequest",
    "AssessmentSubmitRequest",
    "CaseLoadRequest",
    "LmCompleteRequest",
    "ProposalApplyRequest",
    "ProposalSubmitRequest",
    "SandboxExecRequest",
    "SeamJsonRpcRequest",
    "SeamRequestMethod",
    "StageRunProposeRequest",
    "StageRunRequest",
]
