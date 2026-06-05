"""Tests for generated public-seam expression records."""

import msgspec
import pytest
from msgspec import UNSET

from leaven._seam._wire.expressions import (
    PlanExpressionLiteral,
    PlanExpressionProject,
    PreconditionSchemaValid,
    ValueExprExtract,
    ValueExprLiteral,
    ValueExprVar,
)
from leaven._seam._wire.payloads import PlanDocument


def test_schema_valid_precondition_decodes_typed_value_expr() -> None:
    """Example: schema_valid.value is a tagged ValueExpr, not a raw object."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"write","name":"proposal_batch",'
        b'"write":{"kind":"submit_proposal_batch","semantics":"sequence","proposals":[]},'
        b'"preconditions":[{"kind":"schema_valid","schema_fingerprint":"fp_schema",'
        b'"value":{"kind":"literal","value":"ok"}}]}],'
        b'"return":["proposal_batch"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    assert decoded.ops[0].preconditions is not UNSET
    precondition = decoded.ops[0].preconditions[0]

    assert isinstance(precondition, PreconditionSchemaValid)
    assert isinstance(precondition.value, ValueExprLiteral)
    assert precondition.value.value == "ok"


def test_schema_valid_precondition_rejects_unknown_value_expr_kind() -> None:
    """Boundary check: arbitrary objects cannot pass as ValueExpr."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"write","name":"proposal_batch",'
        b'"write":{"kind":"submit_proposal_batch","semantics":"sequence","proposals":[]},'
        b'"preconditions":[{"kind":"schema_valid","schema_fingerprint":"fp_schema",'
        b'"value":{"kind":"mystery","value":"ok"}}]}],'
        b'"return":["proposal_batch"],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


def test_schema_valid_precondition_decodes_var_value_expr() -> None:
    """Example: named ValueExpr variables keep their variant identity."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"write","name":"proposal_batch",'
        b'"write":{"kind":"submit_proposal_batch","semantics":"sequence","proposals":[]},'
        b'"preconditions":[{"kind":"schema_valid","schema_fingerprint":"fp_schema",'
        b'"value":{"kind":"var","name":"candidate_value"}}]}],'
        b'"return":["proposal_batch"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    assert decoded.ops[0].preconditions is not UNSET
    precondition = decoded.ops[0].preconditions[0]

    assert isinstance(precondition, PreconditionSchemaValid)
    assert isinstance(precondition.value, ValueExprVar)
    assert precondition.value.name == "candidate_value"


def test_project_expression_decodes_recursive_input_expr() -> None:
    """Example: project.input keeps the nested PlanExpression variant."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"projected","expr":{"kind":"project",'
        b'"input":{"kind":"literal","value":{"answer":"42"}},'
        b'"projection":{"field":"answer"}}}],'
        b'"return":["projected"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    expr = decoded.ops[0].expr

    assert isinstance(expr, PlanExpressionProject)
    assert isinstance(expr.input, PlanExpressionLiteral)
    assert expr.input.value == {"answer": "42"}


def test_project_expression_rejects_malformed_recursive_input_expr() -> None:
    """Boundary check: project.input is not an arbitrary object."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"projected","expr":{"kind":"project",'
        b'"input":{"value":{"answer":"42"}},'
        b'"projection":{"field":"answer"}}}],'
        b'"return":["projected"],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


def test_extract_value_expr_decodes_recursive_input_expr() -> None:
    """Example: ValueExpr.extract keeps its nested ValueExpr variant."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"write","name":"proposal_batch",'
        b'"write":{"kind":"submit_proposal_batch","semantics":"sequence","proposals":[]},'
        b'"preconditions":[{"kind":"schema_valid","schema_fingerprint":"fp_schema",'
        b'"value":{"kind":"extract","input":{"kind":"var","name":"candidate_value"},'
        b'"path":"$.answer"}}]}],'
        b'"return":["proposal_batch"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    assert decoded.ops[0].preconditions is not UNSET
    precondition = decoded.ops[0].preconditions[0]

    assert isinstance(precondition, PreconditionSchemaValid)
    assert isinstance(precondition.value, ValueExprExtract)
    assert isinstance(precondition.value.input, ValueExprVar)
    assert precondition.value.input.name == "candidate_value"


def test_extract_value_expr_rejects_malformed_recursive_input_expr() -> None:
    """Boundary check: ValueExpr.extract input is not an arbitrary object."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"write","name":"proposal_batch",'
        b'"write":{"kind":"submit_proposal_batch","semantics":"sequence","proposals":[]},'
        b'"preconditions":[{"kind":"schema_valid","schema_fingerprint":"fp_schema",'
        b'"value":{"kind":"extract","input":{"name":"candidate_value"},'
        b'"path":"$.answer"}}]}],'
        b'"return":["proposal_batch"],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


__all__ = []
