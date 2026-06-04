from typing import Any

import pytest

from leaven.builders.case import CaseBuilder


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

    assert client.request_value["method"] == "leaven/case.load"
    params = client.request_value["params"]
    assert params["plan_id"] == "plancasebuilder001"
    assert params["return"] == ["case_load"]
    assert params["commit"] == {"kind": "no_graph_writes"}
    op = params["ops"][0]
    assert op["kind"] == "let"
    assert op["name"] == "case_load"
    assert op["expr"]["kind"] == "case_query"
    query = op["expr"]["query"]
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

    assert [request["method"] for request in client.requests] == [
        "leaven/case.input",
        "leaven/case.target",
        "leaven/case.metadata",
    ]
    assert [request["params"]["ops"][0]["name"] for request in client.requests] == [
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

    assert client.request_value["method"] == "leaven/case.target"


class FakeCaseSeamClient:
    def __init__(self, *, deny_target: bool = False) -> None:
        self.deny_target = deny_target
        self.request_value: dict[str, Any] = {}
        self.requests: list[dict[str, Any]] = []

    def request(self, request: dict[str, Any]) -> dict[str, Any]:
        self.request_value = request
        self.requests.append(request)
        if self.deny_target and request["method"] == "leaven/case.target":
            raise PermissionError("case target denied")
        return {
            "method": request["method"],
            "primary": {
                "kind": "case_record",
                "case": request["params"]["ops"][0]["expr"]["query"]["case"]["id"],
                "input": {"question": "2 + 2?"},
                "target": {"answer": "4"},
                "metadata": {"split": "validation"},
                "receipt": "caserec_builder",
            },
            "receipts": [{"receipt": "caserec_builder", "call_kind": "case_query"}],
        }
