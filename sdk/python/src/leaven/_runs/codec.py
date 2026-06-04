"""Private JSON codec for persisted Python SDK run results."""

from collections.abc import Mapping
from typing import Literal

from .._seam._wire.json_value import JsonObject, json_object
from ..artifacts.prompt import PromptArtifact
from ..result import Optimized

RUN_RESULT_SCHEMA = "leaven.python.optimized.v1"
ArtifactKind = Literal["prompt"]
type OptimizedRecord = dict[str, object]


def encode_optimized[A](result: Optimized[A]) -> JsonObject:
    """Encode an inspectable optimized result into the private JSON envelope."""
    return json_object(
        {
            "schema": RUN_RESULT_SCHEMA,
            "artifact_kind": _artifact_kind(result.best.artifact),
            "optimized": result.model_dump(mode="json"),
        }
    )


def decode_optimized(envelope: JsonObject) -> Optimized[object]:
    """Decode a persisted optimized result envelope."""
    schema = _required_field(envelope, "schema")
    if schema != RUN_RESULT_SCHEMA:
        raise ValueError(f"unsupported run result schema {schema!r}")
    kind = _artifact_kind_from_json(_required_field(envelope, "artifact_kind"))
    raw = _required_field(envelope, "optimized")
    if not isinstance(raw, dict):
        raise TypeError("persisted run result is missing optimized object")
    decoded = _decode_artifacts(json_object(raw), kind)
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
    decoded["best"] = _decode_candidate(decoded["best"], kind)
    frontier = _optional_field(decoded, "frontier", [])
    if not isinstance(frontier, list):
        raise TypeError("persisted run frontier must be a list")
    decoded["frontier"] = [
        _decode_candidate(candidate, kind) for candidate in frontier
    ]
    return decoded


def _decode_candidate(candidate: object, kind: ArtifactKind) -> OptimizedRecord:
    if not isinstance(candidate, dict):
        raise TypeError("persisted candidate must be an object")
    decoded = dict(candidate)
    artifact = _required_field(decoded, "artifact")
    if kind == "prompt":
        decoded["artifact"] = PromptArtifact.model_validate(artifact)
    return decoded


def _required_field(record: Mapping[str, object], key: str) -> object:
    if key not in record:
        raise KeyError(f"persisted run result is missing {key!r}")
    return record[key]


def _optional_field(record: Mapping[str, object], key: str, default: object) -> object:
    if key not in record:
        return default
    return record[key]


__all__ = ["RUN_RESULT_SCHEMA", "decode_optimized", "encode_optimized"]
