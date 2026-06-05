import json
from typing import Literal

import msgspec
import pytest

from leaven._seam import CaseLoadRequest
from leaven._seam._wire.refs import (
    CaseReadInputValue,
    CaseReadMetadataValue,
    CaseReadTargetValue,
)
from leaven._seam._wire.results import CaseLoadResult, CaseRecordPrimary
from leaven.builders.case import CaseBuilder
from leaven.json_value import JsonObject, JsonValue

CaseMethod = Literal[
    "leaven/case.load",
    "leaven/case.input",
    "leaven/case.target",
    "leaven/case.metadata",
]


async def test_load_uses_bound_public_seam_client() -> None:
    """CaseBuilder.load lowers to the durable public-seam case read routes."""

    client = FakeCaseSeamClient()
    case_builder = CaseBuilder._for_seam(
        client,
        idempotency_prefix="case-builder-test",
        plan_id="plancasebuilder001",
        run_id="run_case_builder",
    )

    case = await case_builder.load(
        "case_sdk",
        include=("input", "target", "metadata"),
    )

    assert client.request_value.method == "leaven/case.load"
    params = _params_object(client.request_value.to_params())
    assert params["plan_id"] == "plancasebuilder001"
    assert params["return"] == ["case_load"]
    assert params["commit"] == {"kind": "no_graph_writes"}
    ops = _json_array(params["ops"])
    op = _json_object(ops[0])
    assert op["kind"] == "let"
    assert op["name"] == "case_load"
    expr = _json_object(op["expr"])
    assert expr["kind"] == "case_query"
    query = _json_object(expr["query"])
    assert query["kind"] == "load"
    assert query["case"] == {"kind": "case", "run": "run_case_builder", "id": "case_sdk"}
    assert query["include"] == ["input", "target", "metadata"]
    assert query["projection_schema"] == "fp_schema_sha256_python_case_projection"
    assert case.id == "case_sdk"
    assert case.input == {"question": "2 + 2?"}
    assert case.target == {"answer": "4"}
    assert case.metadata == {"split": "validation"}


async def test_single_field_methods_use_locked_case_routes() -> None:
    """Single-field case projections use the locked narrow read method names."""

    client = FakeCaseSeamClient()
    case_builder = CaseBuilder._for_seam(client, idempotency_prefix="case-single")

    await case_builder.load("case_input", include=("input",))
    await case_builder.load("case_target", include=("target",))
    await case_builder.load("case_metadata", include=("metadata",))

    assert [request.method for request in client.requests] == [
        "leaven/case.input",
        "leaven/case.target",
        "leaven/case.metadata",
    ]
    assert [_op_name(request) for request in client.requests] == [
        "case_input",
        "case_target",
        "case_metadata",
    ]


async def test_target_denial_propagates_seam_error() -> None:
    """Target authorization failures are not swallowed into empty cases."""

    client = FakeCaseSeamClient(deny_target=True)
    case_builder = CaseBuilder._for_seam(client, idempotency_prefix="case-denied")

    with pytest.raises(PermissionError, match="case target denied"):
        await case_builder.load("case_hidden", include=("target",))

    assert client.request_value.method == "leaven/case.target"


async def test_case_read_scalar_field_fails_public_object_projection() -> None:
    """Regression: public Case fields require object-shaped case projections."""

    client = FakeCaseSeamClient(scalar_input=True)
    case_builder = CaseBuilder._for_seam(client, idempotency_prefix="case-scalar")

    with pytest.raises(TypeError, match="case input read result must be a JSON object"):
        await case_builder.load("case_scalar")


class FakeCaseSeamClient:
    def __init__(self, *, deny_target: bool = False, scalar_input: bool = False) -> None:
        self.deny_target = deny_target
        self.scalar_input = scalar_input
        self.request_value = CaseLoadRequest(
            request_id="unset",
            plan_id="unset",
            case_id="unset",
            include=("input",),
        )
        self.requests: list[CaseLoadRequest] = []

    def case_load(self, request: CaseLoadRequest) -> CaseLoadResult:
        self.request_value = request
        self.requests.append(request)
        method = _case_method(request.method)
        if self.deny_target and method == "leaven/case.target":
            raise PermissionError("case target denied")
        params = _params_object(request.to_params())
        ops = _json_array(params["ops"])
        op = _json_object(ops[0])
        expr = _json_object(op["expr"])
        query = _json_object(expr["query"])
        case = _json_object(query["case"])
        case_id = case["id"]
        assert isinstance(case_id, str)
        input_value = _case_input_value(
            "not-an-object" if self.scalar_input else {"question": "2 + 2?"}
        )
        return CaseLoadResult(
            method=method,
            primary=CaseRecordPrimary(
                kind="case_record",
                case=case_id,
                input=input_value,
                target=_case_target_value({"answer": "4"}),
                metadata=_case_metadata_value({"split": "validation"}),
                receipt="caserec_builder",
                data_classes=["public"],
                replayability="fully_managed",
            ),
            receipts=[],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["public"],
        )


def _case_method(value: str) -> CaseMethod:
    if value == "leaven/case.load":
        return "leaven/case.load"
    if value == "leaven/case.input":
        return "leaven/case.input"
    if value == "leaven/case.target":
        return "leaven/case.target"
    if value == "leaven/case.metadata":
        return "leaven/case.metadata"
    raise AssertionError(f"unexpected case method: {value!r}")


def _json_object(value: JsonValue) -> JsonObject:
    assert isinstance(value, dict)
    return value


def _json_array(value: JsonValue) -> list[JsonValue]:
    assert isinstance(value, list)
    return value


def _op_name(request: CaseLoadRequest) -> JsonValue:
    params = _params_object(request.to_params())
    ops = _json_array(params["ops"])
    op = _json_object(ops[0])
    return op["name"]


def _params_object(params: object) -> JsonObject:
    value = json.loads(msgspec.json.encode(params))
    assert isinstance(value, dict)
    return value


def _case_input_value(value: JsonValue) -> CaseReadInputValue:
    return msgspec.convert(value, type=CaseReadInputValue)


def _case_target_value(value: JsonValue) -> CaseReadTargetValue:
    return msgspec.convert(value, type=CaseReadTargetValue)


def _case_metadata_value(value: JsonValue) -> CaseReadMetadataValue:
    return msgspec.convert(value, type=CaseReadMetadataValue)
