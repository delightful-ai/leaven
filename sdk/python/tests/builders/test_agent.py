"""Tests for `leaven.builders.agent`."""

import json

import msgspec
from pydantic import BaseModel

import leaven as lv
from leaven._handles import WorkspaceHandle
from leaven._receipts import CallReceipt
from leaven._seam import AgentRunRequest
from leaven._seam._wire.payloads import BlobRef as WireBlobRef
from leaven._seam._wire.payloads import Cost
from leaven._seam._wire.refs import ExtensionJsonPayload
from leaven._seam._wire.results import AgentCommandRecord, AgentRunResult, AgentSessionPrimary
from leaven.builders.agent import AgentBuilder
from leaven.json_value import JsonObject, JsonValue


class StructuredStatus(BaseModel):
    status: str


async def test_agent_builder_run_lowers_json_schema_output_contract() -> None:
    """Example: bound `agent.run` carries structured output schema authority."""

    client = FakeAgentSeamClient()
    agent = AgentBuilder._for_seam(
        client,
        candidate_id="cand_agent_builder",
        idempotency_prefix="agent-builder-json-schema",
        plan_id="planagentbuilderjson001",
    )
    schema: JsonObject = {
        "type": "object",
        "properties": {"status": {"type": "string"}},
        "required": ["status"],
        "additionalProperties": False,
    }
    output = lv.output.json_schema(schema)

    session = await agent.run(
        workspace=WorkspaceHandle(
            workspace_id="ws_agent_builder_materialized",
            candidate_id="cand_agent_builder",
            lifetime="manual",
            receipt=CallReceipt(receipt_id="wrec_agent_builder"),
        ),
        instructions=lv.AgentInstructions(task="Return JSON status."),
        output=output,
        input_classes=[lv.data_class.WORKSPACE_FILE],
        forbidden_input_classes=[lv.data_class.WORKSPACE_SECRET],
    )

    params = _params_object(client.request_value.to_params())
    ops = _json_array(params["ops"])
    agent_op = _json_object(_json_object(ops[1])["call"])
    output_wire = _json_object(agent_op["output"])
    assert output_wire == {
        "kind": "json_schema",
        "schema_fingerprint": (
            "fp_schema_sha256_"
            "7c4bb65cda2f8731afa3a99a88289088d4fc8f822ae9a2b4e2a881cc1af9c765"
        ),
        "schema": output.schema_,
    }
    assert agent_op["input_classes"] == [lv.data_class.WORKSPACE_FILE]
    assert agent_op["forbidden_input_classes"] == [lv.data_class.WORKSPACE_SECRET]
    assert session.parsed == {"status": "ok"}


async def test_agent_builder_run_parses_model_backed_json_schema_output() -> None:
    """Example: model-backed agent output owns the parsed result type."""

    client = FakeAgentSeamClient()
    agent = AgentBuilder._for_seam(
        client,
        candidate_id="cand_agent_builder",
        idempotency_prefix="agent-builder-model-schema",
        plan_id="planagentbuildermodel001",
    )

    session = await agent.run(
        workspace=WorkspaceHandle(
            workspace_id="ws_agent_builder_materialized",
            candidate_id="cand_agent_builder",
            lifetime="manual",
            receipt=CallReceipt(receipt_id="wrec_agent_builder"),
        ),
        instructions=lv.AgentInstructions(task="Return JSON status."),
        output=lv.output.json_schema(StructuredStatus),
    )

    assert session.parsed == StructuredStatus(status="ok")
    assert session.parsed.status == "ok"


class FakeAgentSeamClient:
    def __init__(self) -> None:
        self.request_value = AgentRunRequest(
            request_id="unset",
            plan_id="unset",
            candidate="unset",
            workspace="unset",
            instructions={"task": "unset"},
            idempotency_prefix="unset",
        )

    def agent_run(self, request: AgentRunRequest) -> AgentRunResult:
        self.request_value = request
        return AgentRunResult(
            method="leaven/agent.run",
            primary=AgentSessionPrimary(
                kind="agent_session",
                status="completed",
                receipt="agentrec_completion",
                graph_revision="rev_agent_builder",
                data_classes=["public", "transcript.raw"],
                replayability="boundary_managed",
                transcript_ref=WireBlobRef(
                    id="blob_agent_builder_transcript",
                    sha256="a" * 64,
                    bytes=128,
                    data_classes=["transcript.raw"],
                ),
                parsed=_wire_json({"status": "ok"}),
                commands=[
                    AgentCommandRecord(
                        argv=["codex", "exec"],
                        status="completed",
                        receipt="agentrec_completion",
                    )
                ],
                cost=Cost(usd_micro=250_000),
            ),
            receipts=[],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["public"],
        )


def _params_object(params: object) -> JsonObject:
    value = json.loads(msgspec.json.encode(params))
    if not isinstance(value, dict):
        raise TypeError("expected JSON object")
    return value


def _wire_json(value: JsonObject) -> ExtensionJsonPayload:
    return msgspec.convert(value, type=ExtensionJsonPayload)


def _json_array(value: JsonValue) -> list[JsonValue]:
    if not isinstance(value, list):
        raise TypeError("expected JSON array")
    return value


def _json_object(value: JsonValue) -> JsonObject:
    if not isinstance(value, dict):
        raise TypeError("expected JSON object")
    return value
