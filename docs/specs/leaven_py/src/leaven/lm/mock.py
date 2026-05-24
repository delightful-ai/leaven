"""`lv.lm.mock(...)` — deterministic mock LM for tests + dry runs."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Literal

from .config import LmConfig


class MockLm(LmConfig):
    """Mock LM that replays canned responses."""

    provider: Literal["mock"] = "mock"
    responses: list[str]
    """Responses returned in order. After exhaustion, raises."""


def mock(
    *,
    responses: Sequence[str],
    model: str = "mock",
    role: str | None = None,
) -> MockLm:
    """Mock LM provider config builder.

    Each call to `cx.lm.complete(...)` consumes one response from the list,
    in order. Use for deterministic tests of evaluator/runner logic.
    """
    return MockLm(model=model, responses=list(responses), role=role)


__all__ = ["MockLm", "mock"]
