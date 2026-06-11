"""`lv.runtime(...)` — compose workspace, LM(s), agent(s), sandbox, trust, budget, cache.

The runtime is one of the required inputs to Leaven composition. It declares how
the run reaches the outside world. Capability tokens, data-class defaults, and
budget enforcement all derive from the runtime + trust profile.

`lv.runtime(...)` builds a full runtime. `lv.runtime.local(...)` is a convenience
for the minimal local case. `lv.runtime.cache.*` namespaces cache constructors
so `lv.runtime(cache=lv.runtime.cache.off())` reads naturally.
"""

from typing import Literal

from pydantic import BaseModel, ConfigDict

from .agent.config import AgentConfig
from .budget import Budget
from .lm.config import LmConfig
from .lm.mock import mock
from .sandbox.config import SandboxConfig
from .trust import TrustProfile
from .workspace.config import WorkspaceConfig
from .workspace.local import local as workspace_local

CacheBackend = Literal["sqlite_default", "memory_only", "off"]


class Cache(BaseModel):
    """Cache config for the run."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    backend: CacheBackend = "sqlite_default"
    path: str | None = None
    """Cache file path; None = engine default under `.leaven/cache/`."""


class _CacheNamespace:
    """The `lv.runtime.cache.*` namespace."""

    @staticmethod
    def sqlite_default(path: str | None = None) -> Cache:
        """SQLite-backed durable cache (engine default)."""
        return Cache(backend="sqlite_default", path=path)

    @staticmethod
    def memory_only() -> Cache:
        """In-memory only cache; lost between runs."""
        return Cache(backend="memory_only")

    @staticmethod
    def off() -> Cache:
        """No cache; every call re-executes."""
        return Cache(backend="off")


class Runtime(BaseModel):
    """A composed runtime. Pass to `lv.optimize(runtime=...)`."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    workspace: WorkspaceConfig
    lm: LmConfig | list[LmConfig] | dict[str, LmConfig]
    agent: AgentConfig | list[AgentConfig] | dict[str, AgentConfig] | None = None
    sandbox: SandboxConfig | None = None
    trust_profile: TrustProfile = TrustProfile.MANAGED_SANDBOX
    budget: Budget | None = None
    cache_config: Cache | None = None


class _RuntimeBuilder:
    """Callable namespace: `lv.runtime(...)` plus `.local()` and `.cache`."""

    cache: _CacheNamespace = _CacheNamespace()

    def __call__(
        self,
        *,
        workspace: WorkspaceConfig,
        lm: LmConfig | list[LmConfig] | dict[str, LmConfig],
        agent: AgentConfig | list[AgentConfig] | dict[str, AgentConfig] | None = None,
        sandbox: SandboxConfig | None = None,
        trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
        budget: Budget | None = None,
        cache: Cache | None = None,
    ) -> Runtime:
        """Compose a runtime.

        `lm` accepts one config, a list (for fallback ordering), or a dict (for
        role binding: `{"grader": ..., "reflector": ...}`). Same for `agent`.

        `trust_profile` accepts the enum or the string form (`"managed_sandbox"`).

        `cache` defaults to engine-managed SQLite if omitted; pass
        `lv.runtime.cache.off()` to disable.
        """
        profile = (
            trust_profile
            if isinstance(trust_profile, TrustProfile)
            else TrustProfile(trust_profile)
        )
        return Runtime(
            workspace=workspace,
            lm=lm,
            agent=agent,
            sandbox=sandbox,
            trust_profile=profile,
            budget=budget,
            cache_config=cache,
        )

    def local(
        self,
        *,
        budget: Budget | None = None,
        lm: LmConfig | None = None,
    ) -> Runtime:
        """Convenience: a minimal local-machine runtime with a mock LM.

        Useful for smoke tests of authoring code. `lm` defaults to a one-line
        mock; pass a scripted `lv.lm.mock(...)` to drive a deterministic local
        optimization (the host reflects with this LM). Real provider LMs and
        agents must be wired through full `lv.runtime(...)`.
        """
        return self(
            workspace=workspace_local(),
            lm=lm if lm is not None else mock(responses=["[mock]"]),
            trust_profile=TrustProfile.TRUSTED_LOCAL_OPERATOR,
            budget=budget,
        )


runtime = _RuntimeBuilder()
"""The composition entry: `lv.runtime(...)` for full, `lv.runtime.local(...)`
for a minimal local convenience, `lv.runtime.cache.*` for cache config."""

__all__ = ["Cache", "CacheBackend", "Runtime", "runtime"]
