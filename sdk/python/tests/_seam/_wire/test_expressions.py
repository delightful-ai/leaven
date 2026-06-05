"""Tests for generated public-seam expression records."""

import msgspec
import pytest
from msgspec import UNSET

from leaven._seam._wire.expressions import (
    GraphStepFilter,
    PlanExpressionFilter,
    PlanExpressionGraphQuery,
    PlanExpressionLiteral,
    PlanExpressionProject,
    PreconditionSchemaValid,
    PredicateEq,
    ProjectionSummary,
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
        b'"projection":{"kind":"summary","fields":["/answer"]}}}],'
        b'"return":["projected"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    expr = decoded.ops[0].expr

    assert isinstance(expr, PlanExpressionProject)
    assert isinstance(expr.input, PlanExpressionLiteral)
    assert expr.input.value == {"answer": "42"}
    assert isinstance(expr.projection, ProjectionSummary)
    assert expr.projection.fields == ["/answer"]


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


def test_filter_expression_decodes_typed_predicate() -> None:
    """Example: filter.predicate is a tagged Predicate record."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"filtered","expr":{"kind":"filter",'
        b'"input":{"kind":"literal","value":{"visible":true}},'
        b'"predicate":{"kind":"eq","field":"/visible","value":true}}}],'
        b'"return":["filtered"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    expr = decoded.ops[0].expr

    assert isinstance(expr, PlanExpressionFilter)
    assert isinstance(expr.predicate, PredicateEq)
    assert expr.predicate.field == "/visible"
    assert expr.predicate.value is True


def test_filter_expression_rejects_unknown_predicate_kind() -> None:
    """Boundary check: predicates are not arbitrary objects."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"filtered","expr":{"kind":"filter",'
        b'"input":{"kind":"literal","value":{"visible":true}},'
        b'"predicate":{"kind":"maybe","field":"/visible","value":true}}}],'
        b'"return":["filtered"],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


def test_graph_query_decodes_typed_projection_and_steps() -> None:
    """Example: graph query projections and steps are generated records."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"rows","expr":{"kind":"graph_query",'
        b'"source":{"kind":"candidate_set","filter":{"predicate":{"kind":"exists","field":"/score"}}},'
        b'"steps":[{"kind":"filter","predicate":{"kind":"eq","field":"/visible","value":true}}],'
        b'"projection":{"kind":"summary","fields":["/score"]}}}],'
        b'"return":["rows"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    expr = decoded.ops[0].expr

    assert isinstance(expr, PlanExpressionGraphQuery)
    assert isinstance(expr.projection, ProjectionSummary)
    assert expr.steps is not UNSET
    assert isinstance(expr.steps[0], GraphStepFilter)
    assert isinstance(expr.steps[0].predicate, PredicateEq)


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
