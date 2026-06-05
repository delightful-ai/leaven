"""Private batch-builder placeholder for a future real `cx.batch()` surface.

The governing spec still describes batching as a desired ergonomic, but the
Python SDK does not yet own a real batch accumulator/requester binding. Until
that lands, this module must not advertise public batch names.
"""

from types import TracebackType

from .agent import AgentBuilder
from .lm import LmBuilder
from .sandbox import SandboxBuilder
from .workspace import WorkspaceBuilder


class _BatchNotResolvedError(RuntimeError):
    """Raised when accessing a batched result before the batch exits."""


class _BatchBuilder:
    """Private owner for the future `async with cx.batch() as b:` object.

    A real implementation must bind to a parent context requester and return
    typed placeholders that resolve to exact method-specific result types.
    """

    workspace: WorkspaceBuilder
    lm: LmBuilder
    agent: AgentBuilder
    sandbox: SandboxBuilder

    async def __aenter__(self) -> "_BatchBuilder":
        raise RuntimeError("private batch placeholder is not a public SDK surface")

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> None:
        raise RuntimeError("private batch placeholder is not a public SDK surface")


def _batch() -> "_BatchBuilder":
    """Construct the private batch placeholder for future internal wiring only."""
    raise RuntimeError("private batch placeholder is not a public SDK surface")


__all__: list[str] = []
