"""Private JSON codec for persisted Python SDK run results."""

from typing import Literal

import msgspec
from msgspec import Raw, Struct

from .._seam._wire.json_value import JsonArray, JsonObject, JsonValue, json_object
from ..artifacts.prompt import PromptArtifact
from ..result import Optimized

RUN_RESULT_SCHEMA = "leaven.python.optimized.v1"
ArtifactKind = Literal["prompt"]
type DecodedCandidate = dict[str, JsonValue | PromptArtifact]
type OptimizedRecord = dict[str, JsonValue | DecodedCandidate | list[DecodedCandidate]]


class _RunResultEnvelope(Struct, forbid_unknown_fields=True):
    schema: str
    artifact_kind: str
    optimized: Raw


_ENCODER = msgspec.json.Encoder()
_ENVELOPE_DECODER = msgspec.json.Decoder(_RunResultEnvelope)


def encode_optimized[A](result: Optimized[A]) -> JsonObject:
    """Encode an inspectable optimized result into the private JSON envelope."""
    return json_object(msgspec.json.decode(encode_optimized_bytes(result)))


def encode_optimized_bytes[A](result: Optimized[A]) -> bytes:
    """Encode an inspectable optimized result into private JSON bytes."""
    return _ENCODER.encode(
        {
            "schema": RUN_RESULT_SCHEMA,
            "artifact_kind": _artifact_kind(result.best.artifact),
            "optimized": result.model_dump(mode="json"),
        }
    )


def decode_optimized(envelope: JsonObject) -> Optimized[object]:
    """Decode a persisted optimized result envelope."""
    return decode_optimized_bytes(_ENCODER.encode(envelope))


def decode_optimized_bytes(body: bytes) -> Optimized[object]:
    """Decode a persisted optimized result envelope from private JSON bytes."""
    envelope = _ENVELOPE_DECODER.decode(body)
    if envelope.schema != RUN_RESULT_SCHEMA:
        raise ValueError(f"unsupported run result schema {envelope.schema!r}")
    kind = _artifact_kind_from_json(envelope.artifact_kind)
    decoded = _decode_artifacts(json_object(msgspec.json.decode(envelope.optimized)), kind)
    return Optimized[object].model_validate(decoded)


def _artifact_kind(artifact: object) -> ArtifactKind:
    if isinstance(artifact, PromptArtifact):
        return "prompt"
    raise TypeError(f"unsupported persisted artifact type: {type(artifact).__name__}")


def _artifact_kind_from_json(value: object) -> ArtifactKind:
    if value == "prompt":
        return "prompt"
    raise ValueError(f"unsupported persisted artifact kind {value!r}")


def _decode_artifacts(raw: JsonObject, kind: ArtifactKind) -> OptimizedRecord:
    decoded = dict(raw)
    decoded["best"] = _decode_candidate(_required_object(decoded, "best"), kind)
    frontier = _optional_array(decoded, "frontier")
    decoded["frontier"] = [
        _decode_candidate(json_object(candidate), kind) for candidate in frontier
    ]
    return decoded


def _decode_candidate(candidate: JsonObject, kind: ArtifactKind) -> DecodedCandidate:
    decoded = dict(candidate)
    if kind == "prompt":
        decoded["artifact"] = PromptArtifact.model_validate(_required_object(decoded, "artifact"))
    return decoded


def _required_json(record: JsonObject, key: str) -> JsonValue:
    if key not in record:
        raise KeyError(f"persisted run result is missing {key!r}")
    return record[key]


def _required_object(record: JsonObject, key: str) -> JsonObject:
    return json_object(_required_json(record, key))


def _optional_array(record: JsonObject, key: str) -> JsonArray:
    if key not in record:
        return []
    value = record[key]
    if not isinstance(value, list):
        raise TypeError(f"persisted run result field {key!r} must be a list")
    return value


__all__ = [
    "RUN_RESULT_SCHEMA",
    "decode_optimized",
    "decode_optimized_bytes",
    "encode_optimized",
    "encode_optimized_bytes",
]
