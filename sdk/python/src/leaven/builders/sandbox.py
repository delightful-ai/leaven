"""`cx.sandbox.*` — sandboxed command execution with typed output capture."""

from collections.abc import Sequence
from typing import Literal

from pydantic import BaseModel, ConfigDict

from .._handles import WorkspaceHandle
from .._receipts import CallReceipt
from ..output import OutputContract


class SandboxExec(BaseModel):
    """Result of `cx.sandbox.exec(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    exit_code: int
    stdout_ref: str
    """Blob ref for stdout bytes (always captured when stream_policy=blob_refs_only)."""
    stderr_ref: str
    files: dict[str, bytes] | None = None
    """Captured output files when `output=lv.output.files(...)`."""
    cost_usd: float | None = None
    receipt: CallReceipt


StreamPolicy = Literal["blob_refs_only", "live_stream"]


class SandboxBuilder:
    """Sandboxed exec bound to a context. Requires a materialized workspace."""

    async def exec(
        self,
        *,
        workspace: WorkspaceHandle,
        argv: Sequence[str],
        env: dict[str, str] | None = None,
        cwd: str | None = None,
        timeout_s: float | None = None,
        output: OutputContract | None = None,
        stream_policy: StreamPolicy = "blob_refs_only",
        input_classes: Sequence[str] | None = None,
        forbidden_input_classes: Sequence[str] | None = None,
    ) -> SandboxExec:
        """Run a command in the configured sandbox against the workspace.

        Sandbox configuration comes from the runtime's `sandbox=` slot.
        Output captures are bound to the receipt; the engine refuses captures
        outside the contract.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["SandboxBuilder", "SandboxExec", "StreamPolicy"]
