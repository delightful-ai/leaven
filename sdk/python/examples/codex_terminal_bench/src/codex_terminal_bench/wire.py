"""Typed decoders for the external JSON the rollout reads.

The rollout consumes three external JSON shapes: the task verifier's CTRF report,
the agent's ATIF trajectory, and the rollout-outcome blob the runner serializes
for the rubric. Each is decoded into a typed `msgspec` struct (with unknown
fields tolerated) instead of being probed as a raw dict, so the rollout never
guesses at an unparsed value's shape.
"""

import msgspec
from msgspec import Struct


class CtrfSummary(Struct, rename=None):
    """The `results.summary` block of a CTRF report."""

    tests: int = 0
    passed: int = 0
    failed: int = 0


class CtrfTest(Struct, rename=None):
    """One `results.tests[]` entry of a CTRF report."""

    name: str = "unnamed"
    status: str = "unknown"


class CtrfResults(Struct, rename=None):
    """The `results` block of a CTRF report."""

    summary: CtrfSummary = msgspec.field(default_factory=CtrfSummary)
    tests: list[CtrfTest] = msgspec.field(default_factory=list)


class CtrfReport(Struct, rename=None):
    """A CTRF (Common Test Report Format) JSON report from the task verifier."""

    results: CtrfResults = msgspec.field(default_factory=CtrfResults)


class TrajectoryToolCall(Struct, rename=None):
    """One ATIF tool call (only the function name is consumed)."""

    function_name: str = ""


class TrajectoryStep(Struct, rename=None):
    """One ATIF trajectory step (only agent-facing fields are consumed)."""

    source: str = ""
    message: str = ""
    tool_calls: list[TrajectoryToolCall] = msgspec.field(default_factory=list)


class Trajectory(Struct, rename=None):
    """An ATIF trajectory (only its steps are consumed)."""

    steps: list[TrajectoryStep] = msgspec.field(default_factory=list)


class RolloutOutcome(Struct, rename=None):
    """The rollout-evidence blob the runner serializes for the rubric."""

    reward: float
    ctrf_passed: int
    ctrf_total: int
    verifier_output: str = ""
    trajectory_path: str | None = None
    input_tokens: int | None = None
    output_tokens: int | None = None
    cost_usd: float | None = None


_CTRF_DECODER = msgspec.json.Decoder(CtrfReport)
_TRAJECTORY_DECODER = msgspec.json.Decoder(Trajectory)
_OUTCOME_DECODER = msgspec.json.Decoder(RolloutOutcome)
_OUTCOME_ENCODER = msgspec.json.Encoder()


def decode_ctrf(data: bytes) -> CtrfReport:
    """Decode a CTRF report, tolerating unknown fields."""
    return _CTRF_DECODER.decode(data)


def decode_trajectory(data: bytes) -> Trajectory:
    """Decode an ATIF trajectory, tolerating unknown fields."""
    return _TRAJECTORY_DECODER.decode(data)


def decode_outcome(data: bytes) -> RolloutOutcome:
    """Decode the rollout-evidence blob the rubric scores."""
    return _OUTCOME_DECODER.decode(data)


def encode_outcome(outcome: RolloutOutcome) -> str:
    """Encode the rollout-evidence blob for the runner output."""
    return _OUTCOME_ENCODER.encode(outcome).decode()


__all__ = [
    "CtrfReport",
    "CtrfResults",
    "CtrfSummary",
    "CtrfTest",
    "RolloutOutcome",
    "Trajectory",
    "TrajectoryStep",
    "TrajectoryToolCall",
    "decode_ctrf",
    "decode_outcome",
    "decode_trajectory",
    "encode_outcome",
]
