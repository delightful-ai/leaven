"""`cx.agent.*` — agentic run inside a workspace, with typed output contracts."""

import asyncio
import math
from collections.abc import Sequence
from typing import Any, Protocol, cast

from pydantic import BaseModel, ConfigDict

from .._handles import WorkspaceHandle
from .._receipts import CallReceipt
from .._seam import AgentRunRequest
from ..agent_instructions import AgentInstructions
from ..blob_ref import BlobRef
from ..output import FilesOutput, JsonSchemaOutput, OutputContract, TextOutput


class AgentSession(BaseModel):
    """Result of `cx.agent.run(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    transcript_ref: str
    """Blob ref to the full session transcript."""
    transcript: BlobRef | None = None
    """Full transcript blob metadata when the provider reports it."""

    parsed: Any | None = None
    """Parsed structured output when `output=lv.output.json_schema(...)`."""

    final_message: str | None = None
    """Final assistant message text (when output is plain text)."""

    files: dict[str, bytes] | None = None
    """Captured files when `output=lv.output.files(...)`."""

    commands: list[dict[str, Any]]
    """Recorded commands the agent ran in the workspace."""

    cost_usd: float | None = None
    receipt: CallReceipt


class _SeamRequester(Protocol):
    """Small private protocol AgentBuilder needs from the seam client."""

    def request(self, request: dict[str, Any]) -> dict[str, Any]: ...


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
    ) -> AgentBuilder:
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
        result = await asyncio.to_thread(self._client.request, request.to_json_rpc())
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


def _output_to_wire(output: OutputContract | None) -> dict[str, Any]:
    if output is None:
        return {"kind": "final_message", "max_bytes": 512}
    if isinstance(output, TextOutput):
        value: dict[str, Any] = {"kind": "final_message"}
        if output.max_chars is not None:
            value["max_bytes"] = output.max_chars
        return value
    if isinstance(output, FilesOutput):
        return {"kind": "files", "paths": output.paths}
    if isinstance(output, JsonSchemaOutput):
        raise NotImplementedError("AgentBuilder.run does not lower json_schema output yet")
    raise TypeError(f"unsupported agent output contract: {type(output).__name__}")


def _agent_session_from_result(result: dict[str, Any]) -> AgentSession:
    primary = result["primary"]
    transcript_ref = primary.get("transcript_ref") or {}
    transcript = _blob_ref(transcript_ref)
    return AgentSession(
        transcript_ref=transcript_ref.get("id", ""),
        transcript=transcript,
        parsed=primary.get("parsed"),
        final_message=None,
        files=None,
        commands=list(primary["commands"]),
        cost_usd=_cost_usd(primary.get("cost")),
        receipt=CallReceipt(
            receipt_id=primary["receipt"],
            blob_refs=[transcript] if transcript is not None else [],
        ),
    )


def _cost_usd(cost: object) -> float | None:
    if not isinstance(cost, dict):
        return None
    cost_value = cast("dict[str, Any]", cost)
    usd_micro = cost_value.get("usd_micro")
    if isinstance(usd_micro, int | float):
        return float(usd_micro) / 1_000_000
    return None


def _blob_ref(value: object) -> BlobRef | None:
    if not isinstance(value, dict):
        return None
    blob = cast("dict[str, Any]", value)
    blob_id = blob.get("id")
    if not isinstance(blob_id, str) or not blob_id:
        return None
    sha256 = blob.get("sha256")
    byte_count = blob.get("bytes")
    data_classes = blob.get("data_classes")
    return BlobRef(
        blob_id=blob_id,
        sha256=sha256 if isinstance(sha256, str) else None,
        bytes=byte_count if isinstance(byte_count, int) else None,
        data_classes=[
            item for item in data_classes if isinstance(item, str)
        ]
        if isinstance(data_classes, list)
        else [],
    )


__all__ = ["AgentBuilder", "AgentSession"]
