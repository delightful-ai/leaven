from leaven._seam import CaseLoadRequest
from leaven._seam._wire.json_value import json_object


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
    params = request.to_params()
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
