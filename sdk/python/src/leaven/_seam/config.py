"""Private service config records for `leaven seam serve --stdio --config`."""

from dataclasses import dataclass, field
from typing import Any


@dataclass(frozen=True)
class SeamExecutionContext:
    """Execution metadata projected into public-seam receipts."""

    capability_fingerprint: str
    policy_fingerprint: str
    base_revision: str
    started_at: str = "2026-06-02T00:00:00Z"
    completed_at: str = "2026-06-02T00:00:01Z"

    def to_json(self) -> dict[str, str]:
        """Return the service config JSON shape."""
        return {
            "capability_fingerprint": self.capability_fingerprint,
            "policy_fingerprint": self.policy_fingerprint,
            "base_revision": self.base_revision,
            "started_at": self.started_at,
            "completed_at": self.completed_at,
        }


@dataclass(frozen=True)
class CodexCliRuntimeConfig:
    """Configured Codex CLI provider for `leaven/agent.run`."""

    codex_bin: str
    model: str = "gpt-5.4-mini"
    timeout_s: int = 180
    codex_home: str | None = None
    bypass_approvals_and_sandbox: bool = False

    def to_json(self) -> dict[str, Any]:
        """Return the service config JSON shape."""
        return {
            "kind": "codex_cli",
            "codex_bin": self.codex_bin,
            "model": self.model,
            "timeout_s": self.timeout_s,
            "codex_home": self.codex_home,
            "bypass_approvals_and_sandbox": self.bypass_approvals_and_sandbox,
        }


@dataclass(frozen=True)
class MockLmRuntimeConfig:
    """Deterministic LM provider config used when a request does not call LM."""

    text: str = "unused"
    input_tokens: int = 1
    output_tokens: int = 1

    def to_json(self) -> dict[str, Any]:
        """Return the service config JSON shape."""
        return {
            "kind": "mock",
            "responses": [
                {
                    "text": self.text,
                    "input_tokens": self.input_tokens,
                    "output_tokens": self.output_tokens,
                }
            ],
        }


@dataclass(frozen=True)
class OpenAiLmRuntimeConfig:
    """Live OpenAI Responses API provider config for `leaven/lm.complete`."""

    api_key_env: str = "OPENAI_API_KEY"
    base_url: str | None = None
    timeout_s: int | None = None
    max_retries: int | None = None

    def to_json(self) -> dict[str, Any]:
        """Return the service config JSON shape."""
        return {
            "kind": "open_ai",
            "api_key_env": self.api_key_env,
            "base_url": self.base_url,
            "timeout_s": self.timeout_s,
            "max_retries": self.max_retries,
        }


@dataclass(frozen=True)
class MockRunnerStageConfig:
    """Deterministic stage runner config used for durable seam mechanics proofs."""

    text: str = "ok"
    summary: str = "mock runner output"

    def to_json(self) -> dict[str, Any]:
        """Return the service config JSON shape."""
        return {
            "kind": "mock_runner",
            "text": self.text,
            "summary": self.summary,
        }


@dataclass(frozen=True)
class CommandRunnerStageConfig:
    """External JSON-RPC stage worker process config."""

    argv: tuple[str, ...]

    def to_json(self) -> dict[str, Any]:
        """Return the service config JSON shape."""
        return {
            "kind": "command_runner",
            "argv": list(self.argv),
        }


@dataclass(frozen=True)
class LocalWorkspaceConfig:
    """Configured local workspace substrate for public-seam calls."""

    seed_files: dict[str, str] = field(default_factory=dict)
    parent: str | None = None

    def to_json(self) -> dict[str, Any]:
        """Return the service config JSON shape."""
        value: dict[str, Any] = {"seed_files": self.seed_files}
        if self.parent is not None:
            value["parent"] = self.parent
        return value


@dataclass(frozen=True)
class SeamServiceConfig:
    """Full private config document passed to `leaven seam serve --stdio`."""

    context: SeamExecutionContext
    capability: dict[str, Any] | None = None
    agent: CodexCliRuntimeConfig | None = None
    workspace: LocalWorkspaceConfig = field(default_factory=LocalWorkspaceConfig)
    lm: MockLmRuntimeConfig | OpenAiLmRuntimeConfig = field(default_factory=MockLmRuntimeConfig)
    stage: MockRunnerStageConfig | CommandRunnerStageConfig | None = None

    def to_json(self) -> dict[str, Any]:
        """Return the Rust service config JSON shape."""
        return {
            "context": self.context.to_json(),
            "capability": self.capability,
            "workspace": self.workspace.to_json(),
            "agent": self.agent.to_json() if self.agent is not None else {"kind": "none"},
            "lm": self.lm.to_json(),
            "stage": self.stage.to_json() if self.stage is not None else {"kind": "none"},
        }


__all__ = [
    "CodexCliRuntimeConfig",
    "CommandRunnerStageConfig",
    "LocalWorkspaceConfig",
    "MockLmRuntimeConfig",
    "MockRunnerStageConfig",
    "OpenAiLmRuntimeConfig",
    "SeamExecutionContext",
    "SeamServiceConfig",
]
