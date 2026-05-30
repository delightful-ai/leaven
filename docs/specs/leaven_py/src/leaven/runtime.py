"""Runtime — `lv.runtime(...)`, `lv.Runtime`, `lv.environment` (deprecated alias).

`runtime` is the execution substrate. `runtime.agent` is the engine-mediated
executor (runtime config, NOT artifact state): mutating the artifact's
Codex-shaped behavior does not change which agent the runtime spawns.

`runtime` is BOTH the callable `runtime(...)` and carries `.local`/`.acp`
shortcut constructors. `lv.environment(...)` is a DEPRECATED ALIAS that emits a
`DeprecationWarning`.

Governing spec: `docs/specs/leaven_python.md` — Runtime.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence

from pydantic import BaseModel, ConfigDict

from .agent.config import AgentConfig
from .budget import Budget
from .cache import CacheConfig
from .lm.config import LmConfig
from .sandbox.config import SandboxConfig
from .trust import TrustProfile
from .workspace.config import WorkspaceConfig

__all__ = ["Runtime", "environment", "runtime"]


class Runtime(BaseModel):
    """The execution substrate config."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    lm: LmConfig | Sequence[LmConfig] | Mapping[str, LmConfig] | None = None
    agent: AgentConfig | Mapping[str, AgentConfig] | None = None
    sandbox: SandboxConfig | None = None
    workspace: WorkspaceConfig | None = None
    trust_profile: TrustProfile | str = TrustProfile.managed_sandbox
    budget: Budget | None = None
    cache: CacheConfig | None = None


class _Runtime:
    """Callable `runtime(...)` carrying `.local` and `.acp` shortcuts."""

    def __call__(
        self,
        *,
        lm: LmConfig | Sequence[LmConfig] | Mapping[str, LmConfig] | None = None,
        agent: AgentConfig | Mapping[str, AgentConfig] | None = None,
        sandbox: SandboxConfig | None = None,
        workspace: WorkspaceConfig | None = None,
        trust_profile: TrustProfile | str = TrustProfile.managed_sandbox,
        budget: Budget | None = None,
        cache: CacheConfig | None = None,
    ) -> Runtime:
        """Build a `Runtime` from explicit substrate config."""
        raise NotImplementedError("see leaven_python.md — Runtime")

    def local(
        self,
        *,
        budget: Budget | None = None,
        workspace: WorkspaceConfig | None = None,
        lm: LmConfig | Sequence[LmConfig] | Mapping[str, LmConfig] | None = None,
        agent: AgentConfig | Mapping[str, AgentConfig] | None = None,
        sandbox: SandboxConfig | None = None,
        trust_profile: TrustProfile | str = TrustProfile.trusted_local_operator,
        cache: CacheConfig | None = None,
    ) -> Runtime:
        """Convenience: local execution substrate (`lv.runtime.local(...)`)."""
        raise NotImplementedError("see leaven_python.md — Runtime / runtime.local")

    def acp(
        self,
        *,
        worker: str | None = None,
        budget: Budget | None = None,
        workspace: WorkspaceConfig | None = None,
        lm: LmConfig | Sequence[LmConfig] | Mapping[str, LmConfig] | None = None,
        agent: AgentConfig | Mapping[str, AgentConfig] | None = None,
        sandbox: SandboxConfig | None = None,
        trust_profile: TrustProfile | str = TrustProfile.managed_sandbox,
        cache: CacheConfig | None = None,
    ) -> Runtime:
        """Convenience: external-driver ACP substrate (`lv.runtime.acp(...)`).

        HONEST PLACEHOLDER: `worker=` is the external worker invocation, but
        there is NOT YET a bound CLI for it. No `[project.scripts]` entry exists,
        so there is no advertised `leaven serve --stdio` binding; `worker`
        defaults to `None` rather than a bound invocation. When the `leaven` CLI
        is real, this becomes a concrete default.
        """
        raise NotImplementedError("see leaven_python.md — Runtime / runtime.acp")


runtime = _Runtime()


def environment(*args: object, **kwargs: object) -> Runtime:
    """DEPRECATED alias for `runtime(...)`; emits a `DeprecationWarning`.

    Removed in 0.3 (spec lines 862-863).
    """
    raise NotImplementedError(
        "deprecated alias; use lv.runtime(...). See leaven_python.md — Runtime"
    )
