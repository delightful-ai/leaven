"""Output contracts — `lv.output.json(...)`, `lv.output.text(...)`, `lv.output.files(...)`.

Output contracts tell the engine how to parse produced files into
`RolloutResult.output`. `parse_as=` is a pydantic model type the produced JSON
is parsed into. `json` also accepts a bare `parse_as=` for inline judge-scorer
use (spec line 621).

Governing spec: `docs/specs/leaven_python.md` — Rollout / RolloutResult.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

__all__ = ["OutputContract", "files", "json", "text"]


class OutputContract(BaseModel):
    """An immutable output-parsing contract; `kind` discriminates."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    kind: str
    path: str | None = None
    parse_as: type | None = None


def json(*, path: str | None = None, parse_as: type | None = None) -> OutputContract:
    """Parse a produced JSON file (or inline response) into `parse_as`.

    Spec: `lv.output.json(path="output/result.json", parse_as=Answer)`; or a
    bare `lv.output.json(parse_as=Verdict)` inside an agentic scorer.
    """
    raise NotImplementedError("see leaven_python.md — output contracts")


def text(*, path: str | None = None) -> OutputContract:
    """Capture produced text output (optionally from `path`)."""
    raise NotImplementedError("see leaven_python.md — output contracts")


def files(*paths: str, **kwargs: object) -> OutputContract:
    """Capture produced output files by workspace-relative path."""
    raise NotImplementedError("see leaven_python.md — output contracts")
