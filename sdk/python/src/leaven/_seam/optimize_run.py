"""Typed `leaven/optimize.run` wire records for the private seam client.

`leaven/optimize.run` is the client->host optimization dispatch locked by
`docs/specs/public-seam-v1/schemas/leaven.optimize_run.v1.schema.json`. Unlike
the worker-profile callback/dispatch methods, it is not in the exported worker
method table, so these request/result records are hand-authored in the same
`msgspec` struct style the generated wire layer uses. The host owns target
custody for the case manifest; runner stage payloads never carry targets.
"""

from typing import Literal

import msgspec
from msgspec import UNSET, Struct, UnsetType

from ._wire import JsonRpcId
from ._wire.errors import JsonRpcError
from ._wire.refs import WireJsonLiteralDepth8

OPTIMIZE_RUN_METHOD = "leaven/optimize.run"
OPTIMIZE_RUN_SCHEMA = "leaven.optimize_run.v1"

OptimizeObjective = Literal["instance", "objective", "hybrid", "cartesian"]
OptimizeSplit = Literal["train", "validation", "test"]


class ArtifactRecord(Struct, frozen=True, forbid_unknown_fields=True):
    """One typed artifact carried by the optimize-run wire (seed or candidate)."""

    artifact_type: str
    artifact_schema: str
    artifact: dict[str, WireJsonLiteralDepth8]


class OptimizeCase(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    """One target-bearing case in the optimize-run manifest.

    The target rides to the host only; the host serves it to scorer stages
    through capability-gated callbacks and never leaks it into runner payloads.
    """

    case: str
    input: WireJsonLiteralDepth8
    target: WireJsonLiteralDepth8
    metadata: dict[str, WireJsonLiteralDepth8] | UnsetType = UNSET
    split: OptimizeSplit | UnsetType = UNSET


class OptimizerConfigDocument(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    """Optimizer knobs honored by the host GEPA loop."""

    max_metric_calls: int
    objective: OptimizeObjective
    population_size: int | UnsetType = UNSET
    minibatch_size: int | UnsetType = UNSET
    max_cost_usd_micro: int | UnsetType = UNSET


class ReflectionLmConfig(Struct, frozen=True, forbid_unknown_fields=True, tag="lm", tag_field="kind"):
    """LM-backed reflection config: the host reflects with `model`."""

    model: str


class ReflectionAgenticConfig(
    Struct,
    frozen=True,
    forbid_unknown_fields=True,
    tag="agentic",
    tag_field="kind",
):
    """Agentic reflection config.

    The host evolves the artifact through a configured agent runtime instead of
    an LM. V1 executes agentic reflection for the `agent_kit` artifact type (a
    Git-backed kit revised in a materialized workspace); it is refused for the
    `prompt` artifact type, which reflects with an LM.
    """


type ReflectionConfig = ReflectionLmConfig | ReflectionAgenticConfig


class OptimizeRunRequestDocument(Struct, frozen=True, forbid_unknown_fields=True):
    """Client->host `leaven/optimize.run` request document."""

    schema_version: Literal["leaven.optimize_run.v1"]
    message: Literal["optimize_run_request"]
    run_id: str
    seed: ArtifactRecord
    cases: list[OptimizeCase]
    optimizer: OptimizerConfigDocument
    reflection: ReflectionConfig
    capability_fingerprint: str


class CandidateEntry(Struct, frozen=True, forbid_unknown_fields=True):
    """One frontier candidate in the optimize-run result."""

    candidate: str
    parent: str | None
    score: float
    artifact: ArtifactRecord


class CostDocument(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    """Aggregate optimization cost reported by the host."""

    usd_micro: int | UnsetType = UNSET
    lm_calls: int | UnsetType = UNSET
    input_tokens: int | UnsetType = UNSET
    output_tokens: int | UnsetType = UNSET
    metric_calls: int | UnsetType = UNSET
    agent_calls: int | UnsetType = UNSET
    sandbox_calls: int | UnsetType = UNSET
    wall_ms: int | UnsetType = UNSET


class RunReference(Struct, frozen=True, forbid_unknown_fields=True):
    """Durable run/revision reference for readback."""

    run: str
    revision: str


class OptimizeRunResultDocument(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    """Host->client `leaven/optimize.run` result document."""

    schema_version: Literal["leaven.optimize_run.v1"]
    message: Literal["optimize_run_result"]
    best: CandidateEntry
    frontier: list[CandidateEntry]
    iterations: int
    metric_calls_used: int
    cost: CostDocument
    run: RunReference
    applied_proposals: list[str] = []


class OptimizeRunRequestEnvelope(Struct, frozen=True, forbid_unknown_fields=True):
    """JSON-RPC request envelope for the unlocked `leaven/optimize.run` method."""

    method: Literal["leaven/optimize.run"]
    params: OptimizeRunRequestDocument
    id: JsonRpcId
    jsonrpc: Literal["2.0"] = "2.0"


class OptimizeRunResponseEnvelope(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    """JSON-RPC response envelope for `leaven/optimize.run`."""

    jsonrpc: Literal["2.0"]
    id: JsonRpcId
    result: OptimizeRunResultDocument | UnsetType = UNSET
    error: JsonRpcError | UnsetType = UNSET


_REQUEST_ENCODER = msgspec.json.Encoder()
_RESPONSE_DECODER = msgspec.json.Decoder(OptimizeRunResponseEnvelope)


def encode_optimize_run_request(
    *,
    request_id: JsonRpcId,
    document: OptimizeRunRequestDocument,
) -> bytes:
    """Encode one `leaven/optimize.run` JSON-RPC request line."""
    return _REQUEST_ENCODER.encode(
        OptimizeRunRequestEnvelope(
            method="leaven/optimize.run",
            params=document,
            id=request_id,
        )
    )


def decode_optimize_run_response(body: bytes) -> OptimizeRunResponseEnvelope:
    """Decode one `leaven/optimize.run` JSON-RPC response envelope."""
    return _RESPONSE_DECODER.decode(body)


__all__ = [
    "OPTIMIZE_RUN_METHOD",
    "OPTIMIZE_RUN_SCHEMA",
    "ArtifactRecord",
    "CandidateEntry",
    "CostDocument",
    "OptimizeCase",
    "OptimizeObjective",
    "OptimizeRunRequestDocument",
    "OptimizeRunResultDocument",
    "OptimizeSplit",
    "OptimizerConfigDocument",
    "ReflectionAgenticConfig",
    "ReflectionConfig",
    "ReflectionLmConfig",
    "RunReference",
    "decode_optimize_run_response",
    "encode_optimize_run_request",
]
