"""Tests for generated public-seam expression records."""

import msgspec
import pytest
from msgspec import UNSET

from leaven._seam._wire.expressions import (
    CaseQueryLoad,
    CaseQueryResolveSet,
    EvaluationSetCases,
    ExtensionObjectExpression,
    GraphSourceCosts,
    GraphSourceEvents,
    GraphSourceExtension,
    GraphStepFilter,
    PlanExpressionCaseQuery,
    PlanExpressionFilter,
    PlanExpressionGraphQuery,
    PlanExpressionLiteral,
    PlanExpressionProject,
    PlanExpressionWorkspaceQuery,
    PreconditionSchemaValid,
    PredicateEq,
    ProjectionArtifact,
    ProjectionSummary,
    ValueExprExtension,
    ValueExprExtract,
    ValueExprLiteral,
    ValueExprVar,
    WorkspaceQueryGitDiff,
    WorkspaceQueryReadFile,
)
from leaven._seam._wire.payloads import PlanDocument
from leaven._seam._wire.refs import (
    ArtifactSelectorJsonPointer,
    CandidateCostScope,
    ExtensionSummaryPayload,
)


def test_schema_valid_precondition_decodes_typed_value_expr() -> None:
    """Example: schema_valid.value is a tagged ValueExpr, not a raw object."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"write","name":"proposal_batch",'
        b'"write":{"kind":"submit_proposal_batch","semantics":"sequence","proposals":[]},'
        b'"preconditions":[{"kind":"schema_valid","schema_fingerprint":"fp_schema",'
        b'"value":{"kind":"literal","value":{"answer":["ok",{"source":"case"}]}}}]}],'
        b'"return":["proposal_batch"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    assert decoded.ops[0].preconditions is not UNSET
    precondition = decoded.ops[0].preconditions[0]

    assert isinstance(precondition, PreconditionSchemaValid)
    assert isinstance(precondition.value, ValueExprLiteral)
    assert precondition.value.value == {"answer": ["ok", {"source": "case"}]}


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
        b'"input":{"kind":"literal","value":{"answer":["42",{"unit":"text"}]}},'
        b'"projection":{"kind":"summary","fields":["/answer"]}}}],'
        b'"return":["projected"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    expr = decoded.ops[0].expr

    assert isinstance(expr, PlanExpressionProject)
    assert isinstance(expr.input, PlanExpressionLiteral)
    assert expr.input.value == {"answer": ["42", {"unit": "text"}]}
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
        b'"predicate":{"kind":"eq","field":"/visible","value":{"flags":[true,{"source":"graph"}]}}}}],'
        b'"return":["filtered"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    expr = decoded.ops[0].expr

    assert isinstance(expr, PlanExpressionFilter)
    assert isinstance(expr.predicate, PredicateEq)
    assert expr.predicate.field == "/visible"
    assert expr.predicate.value == {"flags": [True, {"source": "graph"}]}


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


def test_graph_query_decodes_projection_selector_and_cost_scope_owners() -> None:
    """Example: selector and cost scope leaves keep expression-specific owners."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"artifact_view","expr":{"kind":"graph_query",'
        b'"source":{"kind":"by_candidate","candidate":"cand_alpha"},'
        b'"projection":{"kind":"artifact_projection","artifact":{'
        b'"surface_fingerprint":"fp_surface_sha256_prompt",'
        b'"projection_schema":"fp_schema_sha256_projection",'
        b'"selector_schema":"fp_schema_sha256_selector",'
        b'"selector":{"kind":"json_pointer","path":"/prompt/0"},'
        b'"data_classes":["candidate.artifact"]}}}},'
        b'{"kind":"let","name":"costs","expr":{"kind":"graph_query",'
        b'"source":{"kind":"costs","scope":{"kind":"candidate",'
        b'"candidate":"cand_alpha","dimensions":["lm","usd_micro"]}},'
        b'"projection":{"kind":"summary"}}}],'
        b'"return":["artifact_view","costs"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    artifact_expr = decoded.ops[0].expr
    costs_expr = decoded.ops[1].expr

    assert isinstance(artifact_expr, PlanExpressionGraphQuery)
    assert isinstance(artifact_expr.projection, ProjectionArtifact)
    assert isinstance(artifact_expr.projection.artifact.selector, ArtifactSelectorJsonPointer)
    assert artifact_expr.projection.artifact.selector.path == "/prompt/0"
    assert isinstance(costs_expr, PlanExpressionGraphQuery)
    assert isinstance(costs_expr.source, GraphSourceCosts)
    assert isinstance(costs_expr.source.scope, CandidateCostScope)
    assert costs_expr.source.scope.candidate == "cand_alpha"
    assert costs_expr.source.scope.dimensions == ["lm", "usd_micro"]


def test_graph_query_rejects_open_selector_and_cost_scope_payloads() -> None:
    """Regression: selector and cost scope leaves are closed records."""

    open_selector = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"artifact_view","expr":{"kind":"graph_query",'
        b'"source":{"kind":"by_candidate","candidate":"cand_alpha"},'
        b'"projection":{"kind":"artifact_projection","artifact":{'
        b'"surface_fingerprint":"fp_surface_sha256_prompt",'
        b'"projection_schema":"fp_schema_sha256_projection",'
        b'"selector":{"path":["prompt",{"segment":0}]}}}}}],'
        b'"return":["artifact_view"],"commit":{"kind":"no_graph_writes"}}'
    )
    open_scope = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"costs","expr":{"kind":"graph_query",'
        b'"source":{"kind":"costs","scope":{"kind":"candidate",'
        b'"candidate":"cand_alpha","dimensions":["lm",{"unit":"usd_micro"}]}},'
        b'"projection":{"kind":"summary"}}}],'
        b'"return":["costs"],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(open_selector, type=PlanDocument)
    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(open_scope, type=PlanDocument)


def test_graph_query_event_filter_decodes_owned_json() -> None:
    """Example: event graph-source filters use a named bounded JSON owner."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"events","expr":{"kind":"graph_query",'
        b'"source":{"kind":"events","since_revision":"rev_1",'
        b'"filter":{"kind":"run_context","data_classes":["event.public",{"scope":"run"}]}},'
        b'"projection":{"kind":"summary"}}}],'
        b'"return":["events"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    events_expr = decoded.ops[0].expr

    assert isinstance(events_expr, PlanExpressionGraphQuery)
    assert isinstance(events_expr.source, GraphSourceEvents)
    assert events_expr.source.filter == {
        "kind": "run_context",
        "data_classes": ["event.public", {"scope": "run"}],
    }


def test_graph_query_event_filter_rejects_non_object_filter() -> None:
    """Boundary check: event filters are object-shaped JSON, not arbitrary values."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"events","expr":{"kind":"graph_query",'
        b'"source":{"kind":"events","filter":["run_context"]},'
        b'"projection":{"kind":"summary"}}}],'
        b'"return":["events"],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


def test_workspace_query_decodes_typed_filesystem_op() -> None:
    """Example: workspace_query.op is a tagged workspace operation record."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"workspace_file",'
        b'"expr":{"kind":"workspace_query","workspace":"ws_1",'
        b'"op":{"kind":"read_file","path":"README.md",'
        b'"expected_data_classes":["public"],"max_bytes":4096}}}],'
        b'"return":["workspace_file"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    expr = decoded.ops[0].expr

    assert isinstance(expr, PlanExpressionWorkspaceQuery)
    assert isinstance(expr.op, WorkspaceQueryReadFile)
    assert expr.op.path == "README.md"
    assert expr.op.expected_data_classes == ["public"]
    assert expr.op.max_bytes == 4096


def test_workspace_query_decodes_typed_git_op() -> None:
    """Example: git operations keep their locked operation variant."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"workspace_diff",'
        b'"expr":{"kind":"workspace_query","workspace":"ws_1",'
        b'"op":{"kind":"git_diff","against":"seed"}}}],'
        b'"return":["workspace_diff"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    expr = decoded.ops[0].expr

    assert isinstance(expr, PlanExpressionWorkspaceQuery)
    assert isinstance(expr.op, WorkspaceQueryGitDiff)
    assert expr.op.against == "seed"


def test_workspace_query_rejects_unknown_op_kind() -> None:
    """Boundary check: workspace ops are not arbitrary JSON objects."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"workspace_unknown",'
        b'"expr":{"kind":"workspace_query","workspace":"ws_1",'
        b'"op":{"kind":"rm_rf","path":"."}}}],'
        b'"return":["workspace_unknown"],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


def test_case_query_decodes_typed_load_query() -> None:
    """Example: case_query.query load bodies are tagged records."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"case_load",'
        b'"expr":{"kind":"case_query","query":{"kind":"load",'
        b'"case":{"kind":"case","run":"run_1","id":"case_1"},'
        b'"include":["input","metadata"],"projection_schema":"fp_schema"}}}],'
        b'"return":["case_load"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    expr = decoded.ops[0].expr

    assert isinstance(expr, PlanExpressionCaseQuery)
    assert isinstance(expr.query, CaseQueryLoad)
    assert expr.query.include == ["input", "metadata"]
    assert expr.query.projection_schema == "fp_schema"


def test_case_query_decodes_typed_resolve_set_query() -> None:
    """Example: resolve_set reuses the shared EvaluationSetExpr owner."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"cases",'
        b'"expr":{"kind":"case_query","query":{"kind":"resolve_set",'
        b'"set":{"kind":"cases","cases":["case_1"],"requires_partition_resolution":true},'
        b'"purpose":"validation"}}}],'
        b'"return":["cases"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    expr = decoded.ops[0].expr

    assert isinstance(expr, PlanExpressionCaseQuery)
    assert isinstance(expr.query, CaseQueryResolveSet)
    assert isinstance(expr.query.set, EvaluationSetCases)
    assert expr.query.set.requires_partition_resolution is True


def test_case_query_rejects_unresolved_case_sets() -> None:
    """Boundary check: schema-required partition resolution cannot be false."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"cases",'
        b'"expr":{"kind":"case_query","query":{"kind":"resolve_set",'
        b'"set":{"kind":"cases","cases":["case_1"],"requires_partition_resolution":false},'
        b'"purpose":"validation"}}}],'
        b'"return":["cases"],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


def test_case_query_rejects_unknown_query_kind() -> None:
    """Boundary check: case_query.query is not an arbitrary object."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"cases",'
        b'"expr":{"kind":"case_query","query":{"kind":"lookup","case":"case_1"}}}],'
        b'"return":["cases"],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


def test_extension_expression_decodes_nested_payload() -> None:
    """Example: extension expression payload is a closed typed record."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"extension_value",'
        b'"expr":{"kind":"extension","namespace":"x.test","op":"literal_payload",'
        b'"schema_fingerprint":"fp_schema","payload":{"kind":"summary",'
        b'"summary":"route a b","data_classes":["public"]}}}],'
        b'"return":["extension_value"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    expr = decoded.ops[0].expr

    assert isinstance(expr, ExtensionObjectExpression)
    assert isinstance(expr.payload, ExtensionSummaryPayload)
    assert expr.payload.summary == "route a b"
    assert expr.payload.data_classes == ["public"]


def test_extension_expression_rejects_open_payload() -> None:
    """Regression: ExtensionObject payloads are not arbitrary JSON islands."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"extension_value",'
        b'"expr":{"kind":"extension","namespace":"x.test","op":"literal_payload",'
        b'"schema_fingerprint":"fp_schema","payload":{"route":["a",{"b":[1,2]}]}}}],'
        b'"return":["extension_value"],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


def test_graph_extension_source_decodes_nested_payload() -> None:
    """Example: graph source extensions use the same typed payload owner."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"rows","expr":{"kind":"graph_query",'
        b'"source":{"kind":"extension","namespace":"x.graph","op":"source",'
        b'"schema_fingerprint":"fp_schema","payload":{"kind":"summary",'
        b'"summary":"cursor r 1"}},'
        b'"projection":{"kind":"summary","fields":["/score"]}}}],'
        b'"return":["rows"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    expr = decoded.ops[0].expr

    assert isinstance(expr, PlanExpressionGraphQuery)
    assert isinstance(expr.source, GraphSourceExtension)
    assert isinstance(expr.source.payload, ExtensionSummaryPayload)
    assert expr.source.payload.summary == "cursor r 1"


def test_extension_value_expr_decodes_nested_payload() -> None:
    """Example: ValueExpr extension payloads are closed typed records."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"write","name":"proposal_batch",'
        b'"write":{"kind":"submit_proposal_batch","semantics":"sequence","proposals":[]},'
        b'"preconditions":[{"kind":"schema_valid","schema_fingerprint":"fp_schema",'
        b'"value":{"kind":"extension","namespace":"x.value","op":"payload",'
        b'"schema_fingerprint":"fp_schema","payload":{"kind":"summary",'
        b'"summary":"checks ok"}}}]}],'
        b'"return":["proposal_batch"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    assert decoded.ops[0].preconditions is not UNSET
    precondition = decoded.ops[0].preconditions[0]

    assert isinstance(precondition, PreconditionSchemaValid)
    assert isinstance(precondition.value, ValueExprExtension)
    assert isinstance(precondition.value.payload, ExtensionSummaryPayload)
    assert precondition.value.payload.summary == "checks ok"


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
