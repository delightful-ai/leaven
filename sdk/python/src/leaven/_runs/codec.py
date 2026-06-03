"""Private JSON codec for persisted Python SDK run results."""

from __future__ import annotations

from typing import Any, Literal, cast

from ..artifacts.prompt import PromptArtifact
from ..result import Optimized

RUN_RESULT_SCHEMA = "leaven.python.optimized.v1"
ArtifactKind = Literal["prompt"]


def encode_optimized(result: Optimized[Any]) -> dict[str, Any]:
    """Encode an inspectable optimized result into the private JSON envelope."""
    return {
        "schema": RUN_RESULT_SCHEMA,
        "artifact_kind": _artifact_kind(result.best.artifact),
        "optimized": result.model_dump(mode="json"),
    }


def decode_optimized(envelope: dict[str, Any]) -> Optimized[Any]:
    """Decode a persisted optimized result envelope."""
    schema = envelope.get("schema")
    if schema != RUN_RESULT_SCHEMA:
        raise ValueError(f"unsupported run result schema {schema!r}")
    kind = cast("ArtifactKind", envelope.get("artifact_kind"))
    raw = envelope.get("optimized")
    if not isinstance(raw, dict):
        raise ValueError("persisted run result is missing optimized object")
    decoded = _decode_artifacts(raw, kind)
    return Optimized[Any].model_validate(decoded)


def _artifact_kind(artifact: Any) -> ArtifactKind:
    if isinstance(artifact, PromptArtifact):
        return "prompt"
    raise TypeError(f"unsupported persisted artifact type: {type(artifact).__name__}")


def _decode_artifacts(raw: dict[str, Any], kind: ArtifactKind) -> dict[str, Any]:
    decoded = dict(raw)
    decoded["best"] = _decode_candidate(decoded["best"], kind)
    decoded["frontier"] = [
        _decode_candidate(candidate, kind) for candidate in decoded.get("frontier", [])
    ]
    return decoded


def _decode_candidate(candidate: object, kind: ArtifactKind) -> dict[str, Any]:
    if not isinstance(candidate, dict):
        raise ValueError("persisted candidate must be an object")
    decoded = dict(candidate)
    artifact = decoded.get("artifact")
    if kind == "prompt":
        decoded["artifact"] = PromptArtifact.model_validate(artifact)
    return decoded


__all__ = ["RUN_RESULT_SCHEMA", "decode_optimized", "encode_optimized"]
