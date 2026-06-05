"""`cx.agent.*` — agentic run inside a workspace, with typed output contracts."""

import asyncio
import math
from collections.abc import Sequence
from typing import Protocol

from msgspec import UNSET, UnsetType
from pydantic import BaseModel, ConfigDict

from .._handles import WorkspaceHandle
from .._receipts import CallReceipt
from .._seam import AgentRunRequest
from .._seam._wire import JsonObject
from .._seam._wire.json_value import json_object
from .._seam._wire.payloads import BlobRef as WireBlobRef
from .._seam._wire.payloads import Cost
from .._seam._wire.results import AgentRunResult
from ..agent_instructions import AgentInstructions
from ..blob_ref import BlobRef
from ..json_value import JsonValue
from ..output import FilesOutput, JsonSchemaOutput, OutputContract, TextOutput
from ._output_contract import json_schema_output_to_wire


class AgentCommand(BaseModel):
    """One command audited inside an agent session."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    argv: list[str]
    status: str
    receipt: str | None = None


class AgentSession(BaseModel):
    """Result of `cx.agent.run(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    transcript_ref: str
    """Blob ref to the full session transcript."""
    transcript: BlobRef | None = None
    """Full transcript blob metadata when the provider reports it."""

    parsed: JsonValue | None = None
    """Parsed structured output when `output=lv.output.json_schema(...)`."""

    final_message: str | None = None
    """Final assistant message text (when output is plain text)."""

    files: dict[str, bytes] | None = None
    """Captured files when `output=lv.output.files(...)`."""

    commands: list[AgentCommand]
    """Recorded commands the agent ran in the workspace."""

    cost_usd: float | None = None
    receipt: CallReceipt


class _SeamRequester(Protocol):
    """Small private protocol AgentBuilder needs from the seam client."""

    def agent_run(self, request: AgentRunRequest) -> AgentRunResult: ...


class AgentBuilder:
    """Agent runs bound to a context. Requires a materialized workspace."""

    def __init__(
        self,
        *,
        _client: _SeamRequester | None = None,
        _candidate_id: str | None = None,
        _idempotency_prefix: str = "agent-builder",
        _plan_id: str = "planpythonagentbuilder001",
    ) -> None:
        self._client = _client
        self._candidate_id = _candidate_id
        self._idempotency_prefix = _idempotency_prefix
        self._plan_id = _plan_id

    @classmethod
    def _for_seam(
        cls,
        client: _SeamRequester,
        *,
        candidate_id: str,
        idempotency_prefix: str = "agent-builder",
        plan_id: str = "planpythonagentbuilder001",
    ) -> "AgentBuilder":
        """Bind this builder to the private public-seam process client."""
        return cls(
            _client=client,
            _candidate_id=candidate_id,
            _idempotency_prefix=idempotency_prefix,
            _plan_id=plan_id,
        )

    async def run(
        self,
        *,
        workspace: WorkspaceHandle,
        instructions: AgentInstructions,
        runtime: str | None = None,
        output: OutputContract | None = None,
        timeout_s: float | None = None,
        allowed_commands: Sequence[str] | None = None,
        input_classes: Sequence[str] | None = None,
        forbidden_input_classes: Sequence[str] | None = None,
    ) -> AgentSession:
        """Run an agent session against the workspace.

        `runtime` selects a configured agent (default if only one is configured;
        explicit when multiple are wired). `output` constrains the structured
        return; without it the session text is the result.
        """
        if self._client is None:
            raise NotImplementedError(
                "AgentBuilder.run needs an engine-bound public-seam client; "
                "use the cx.agent instance supplied to a running stage"
            )

        candidate_id = workspace.candidate_id or self._candidate_id
        if candidate_id is None:
            raise ValueError("AgentBuilder.run requires a workspace with a candidate_id")
        if forbidden_input_classes is not None:
            raise NotImplementedError("AgentBuilder.run does not lower forbidden_input_classes yet")

        request = AgentRunRequest(
            request_id=f"{self._idempotency_prefix}-agent-run",
            plan_id=self._plan_id,
            candidate=candidate_id,
            workspace=workspace.workspace_id,
            instructions=_instructions_to_wire(instructions),
            idempotency_prefix=self._idempotency_prefix,
            runtime=runtime or "codex-cli",
            timeout_s=_timeout_seconds(timeout_s),
            output=_output_to_wire(output),
            allowed_commands=allowed_commands,
            input_classes=input_classes,
        )
        result = await asyncio.to_thread(self._client.agent_run, request)
        return _agent_session_from_result(result)


def _instructions_to_wire(instructions: AgentInstructions) -> dict[str, str]:
    value = {"task": instructions.task}
    if instructions.system is not None:
        value["system"] = instructions.system
    if instructions.rubric is not None:
        value["rubric"] = instructions.rubric
    return value


def _timeout_seconds(timeout_s: float | None) -> int:
    return max(1, math.ceil(timeout_s)) if timeout_s is not None else 180


def _output_to_wire(output: OutputContract | None) -> JsonObject:
    if output is None:
        return {"kind": "final_message", "max_bytes": 512}
    if isinstance(output, TextOutput):
        value: JsonObject = {"kind": "final_message"}
        if output.max_chars is not None:
            value["max_bytes"] = output.max_chars
        return value
    if isinstance(output, FilesOutput):
        return json_object({"kind": "files", "paths": output.paths})
    if isinstance(output, JsonSchemaOutput):
        return json_schema_output_to_wire(output)
    raise TypeError(f"unsupported agent output contract: {type(output).__name__}")


def _agent_session_from_result(result: AgentRunResult) -> AgentSession:
    primary = result.primary
    transcript = _blob_ref(primary.transcript_ref)
    return AgentSession(
        transcript_ref=transcript.blob_id if transcript is not None else "",
        transcript=transcript,
        parsed=None,
        final_message=None,
        files=None,
        commands=[
            AgentCommand(
                argv=list(command.argv),
                status=command.status,
                receipt=None if command.receipt is UNSET else command.receipt,
            )
            for command in primary.commands
        ],
        cost_usd=_cost_usd(primary.cost),
        receipt=CallReceipt(
            receipt_id=primary.receipt,
            blob_refs=[] if transcript is None else [transcript],
        ),
    )


def _cost_usd(cost: Cost | UnsetType) -> float | None:
    if cost is UNSET:
        return None
    usd_micro = cost.usd_micro
    return None if usd_micro is UNSET else usd_micro / 1_000_000


def _blob_ref(value: WireBlobRef | UnsetType) -> BlobRef | None:
    if value is UNSET:
        return None
    return BlobRef(
        blob_id=value.id,
        sha256=value.sha256,
        bytes=value.bytes,
        data_classes=list(value.data_classes),
    )


__all__ = ["AgentBuilder", "AgentCommand", "AgentSession"]
