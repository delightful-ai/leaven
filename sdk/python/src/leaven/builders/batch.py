"""`cx.batch()` — accumulate multiple ops into one Plan IR document.

The batch context manager is the load-bearing ergonomic: it lets the user
write multiple operations as if they were independent calls, while the wire
treats them as one transaction with one receipt root. Without it, every
effect is a separate round-trip.

Geometry (per `docs/specs/leaven_python.md` "The Python authoring surface"):

    async with cx.batch() as b:
        diff = b.workspace.git_diff(ws, against="parent")
        tests = b.sandbox.exec(workspace=ws, argv=[...])
        agent = b.agent.run(workspace=ws, instructions=...)
    # After the block: diff/tests/agent are real values (placeholders
    # resolved on `__aexit__`).

The placeholders accessed inside the block raise `BatchNotResolvedError` on
attribute access; outside the block they proxy to the real result.
"""

from types import TracebackType

from .agent import AgentBuilder
from .lm import LmBuilder
from .sandbox import SandboxBuilder
from .workspace import WorkspaceBuilder


class BatchNotResolvedError(RuntimeError):
    """Raised when accessing a batched result before the `async with` block exits."""


class BatchBuilder:
    """The `b` object inside `async with cx.batch() as b:`.

    Exposes the same builder namespaces as the parent context (`b.workspace`,
    `b.lm`, `b.agent`, `b.sandbox`) but the calls return placeholder objects
    that resolve when the context manager exits.
    """

    workspace: WorkspaceBuilder
    lm: LmBuilder
    agent: AgentBuilder
    sandbox: SandboxBuilder

    async def __aenter__(self) -> "BatchBuilder":
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> None:
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


def batch() -> "BatchBuilder":
    """Construct a fresh batch builder; usually accessed as `cx.batch()`."""
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["BatchBuilder", "BatchNotResolvedError", "batch"]
