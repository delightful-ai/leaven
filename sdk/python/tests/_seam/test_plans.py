import json

import msgspec

from leaven._seam import CaseLoadRequest
from leaven._seam._wire import JsonObject
from leaven._seam._wire.calls import AgentRunCall, LmCompleteCall, WorkspaceMaterializeCall
from leaven._seam._wire.json_value import json_object
from leaven._seam._wire.payloads import PlanDocument
from leaven._seam.lm_plans import LmCompleteRequest
from leaven._seam.plans import AgentRunRequest


def test_case_load_request_names_locked_single_field_routes() -> None:
    assert CaseLoadRequest(
        request_id="case-input",
        plan_id="plan-input",
        case_id="case_input",
        include=("input",),
    ).method == "leaven/case.input"
    assert CaseLoadRequest(
        request_id="case-target",
        plan_id="plan-target",
        case_id="case_target",
        include=("target",),
    ).method == "leaven/case.target"
    assert CaseLoadRequest(
        request_id="case-metadata",
        plan_id="plan-metadata",
        case_id="case_metadata",
        include=("metadata",),
    ).method == "leaven/case.metadata"


def test_case_load_request_uses_composite_route_for_multi_field_projection() -> None:
    request = CaseLoadRequest(
        request_id="case-load",
        plan_id="plancasebuilder001",
        case_id="case_sdk",
        include=("input", "target", "metadata"),
        run_id="run_case_builder",
    )

    assert request.method == "leaven/case.load"
    params = _params_object(request.to_params())
    assert params["schema_version"] == "leaven.plan.v1"
    assert params["plan_id"] == "plancasebuilder001"
    assert params["return"] == ["case_load"]
    assert params["commit"] == {"kind": "no_graph_writes"}
    ops = params["ops"]
    assert isinstance(ops, list)
    op = json_object(ops[0])
    expr = json_object(op["expr"])
    query = json_object(expr["query"])
    assert query["case"] == {"kind": "case", "run": "run_case_builder", "id": "case_sdk"}
    assert query["include"] == ["input", "target", "metadata"]
    assert query["projection_schema"] == "fp_schema_sha256_python_case_projection"


def test_agent_request_params_decode_typed_call_variants() -> None:
    request = AgentRunRequest(
        request_id="agent-test",
        plan_id="plan-agent-test",
        candidate="cand_agent",
        workspace="ws_agent",
        instructions={"task": "change the skill"},
        idempotency_prefix="agent",
        allowed_commands=("codex",),
    )

    decoded = _decode_plan_params(request.to_params())
    workspace = decoded.ops[0].call
    agent = decoded.ops[1].call

    assert isinstance(workspace, WorkspaceMaterializeCall)
    assert workspace.candidate == "cand_agent"
    assert isinstance(agent, AgentRunCall)
    assert agent.instructions.task == "change the skill"
    assert agent.tool_policy is not msgspec.UNSET
    assert agent.tool_policy.allowed_commands == ["codex"]


def test_lm_request_params_decode_typed_call_variant() -> None:
    request = LmCompleteRequest(
        request_id="lm-test",
        plan_id="plan-lm-test",
        idempotency_key="idem-lm-test",
        messages=[{"role": "user", "content": [{"kind": "text", "text": "say ok"}]}],
        model="gpt-test",
        max_tokens=16,
    )

    decoded = _decode_plan_params(request.to_params())
    call = decoded.ops[0].call

    assert isinstance(call, LmCompleteCall)
    assert call.model == "gpt-test"
    assert call.sampling is not msgspec.UNSET
    assert call.sampling.max_output_tokens == 16


def _decode_plan_params(params: object) -> PlanDocument:
    return msgspec.json.decode(msgspec.json.encode(params), type=PlanDocument)


def _params_object(params: object) -> JsonObject:
    value = json.loads(msgspec.json.encode(params))
    assert isinstance(value, dict)
    return value
