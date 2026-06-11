"""Private service config records for `leaven seam serve --stdio --config`."""

from dataclasses import dataclass, field
from typing import Literal

import msgspec
from msgspec import Struct, UnsetType

from ._wire import JsonObject
from ._wire.json_value import json_object
from .capability import CapabilityDocument


class SeamExecutionContextDocument(Struct, frozen=True, forbid_unknown_fields=True):
    """Wire service execution context config."""

    capability_fingerprint: str
    policy_fingerprint: str
    base_revision: str
    started_at: str
    completed_at: str


class NoneProviderConfig(Struct, frozen=True, forbid_unknown_fields=True):
    """Disabled optional service provider."""

    kind: Literal["none"] = "none"


class CodexCliRuntimeDocument(Struct, frozen=True, forbid_unknown_fields=True):
    """Wire Codex CLI provider config."""

    codex_bin: str
    model: str
    timeout_s: int
    codex_home: str | None
    bypass_approvals_and_sandbox: bool
    kind: Literal["codex_cli"] = "codex_cli"


class MockLmResponseDocument(Struct, frozen=True, forbid_unknown_fields=True):
    """One wire mock LM response config."""

    text: str
    input_tokens: int
    output_tokens: int


class MockLmRuntimeDocument(Struct, frozen=True, forbid_unknown_fields=True):
    """Wire mock LM provider config."""

    responses: list[MockLmResponseDocument]
    kind: Literal["mock"] = "mock"


class OpenAiLmRuntimeDocument(Struct, frozen=True, forbid_unknown_fields=True):
    """Wire OpenAI LM provider config."""

    api_key_env: str
    base_url: str | None
    timeout_s: int | None
    max_retries: int | None
    kind: Literal["open_ai"] = "open_ai"


class MockRunnerStageDocument(Struct, frozen=True, forbid_unknown_fields=True):
    """Wire mock runner stage config."""

    text: str
    summary: str
    kind: Literal["mock_runner"] = "mock_runner"


class CommandRunnerStageDocument(Struct, frozen=True, forbid_unknown_fields=True):
    """Wire command runner stage config."""

    argv: list[str]
    kind: Literal["command_runner"] = "command_runner"


class LocalWorkspaceDocument(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    """Wire local workspace config."""

    seed_files: dict[str, str]
    parent: str | None | UnsetType = msgspec.UNSET


type AgentRuntimeDocument = CodexCliRuntimeDocument | NoneProviderConfig
type LmRuntimeDocument = MockLmRuntimeDocument | OpenAiLmRuntimeDocument
type StageRuntimeDocument = MockRunnerStageDocument | CommandRunnerStageDocument | NoneProviderConfig


class SeamServiceDocument(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    """Full wire service config document."""

    context: SeamExecutionContextDocument
    capability: CapabilityDocument | None
    workspace: LocalWorkspaceDocument
    agent: AgentRuntimeDocument
    lm: LmRuntimeDocument
    stage: StageRuntimeDocument
    optimize_runs_root: str | None | UnsetType = msgspec.UNSET


def config_to_json(config: SeamServiceDocument) -> JsonObject:
    """Project typed service config to JSON-compatible builtins."""
    return json_object(msgspec.to_builtins(config))


def config_to_json_bytes(config: SeamServiceDocument) -> bytes:
    """Encode typed service config as JSON bytes for `--config`."""
    return msgspec.json.encode(config)


@dataclass(frozen=True)
class SeamExecutionContext:
    """Execution metadata projected into public-seam receipts."""

    capability_fingerprint: str
    policy_fingerprint: str
    base_revision: str
    started_at: str = "2026-06-02T00:00:00Z"
    completed_at: str = "2026-06-02T00:00:01Z"

    def to_wire(self) -> SeamExecutionContextDocument:
        """Return the typed service config record."""
        return SeamExecutionContextDocument(
            capability_fingerprint=self.capability_fingerprint,
            policy_fingerprint=self.policy_fingerprint,
            base_revision=self.base_revision,
            started_at=self.started_at,
            completed_at=self.completed_at,
        )

    def to_json(self) -> JsonObject:
        """Return the JSON-compatible service config shape."""
        return json_object(msgspec.to_builtins(self.to_wire()))


@dataclass(frozen=True)
class CodexCliRuntimeConfig:
    """Configured Codex CLI provider for `leaven/agent.run`."""

    codex_bin: str
    model: str = "gpt-5.4-mini"
    timeout_s: int = 180
    codex_home: str | None = None
    bypass_approvals_and_sandbox: bool = False

    def to_wire(self) -> CodexCliRuntimeDocument:
        """Return the typed service config record."""
        return CodexCliRuntimeDocument(
            codex_bin=self.codex_bin,
            model=self.model,
            timeout_s=self.timeout_s,
            codex_home=self.codex_home,
            bypass_approvals_and_sandbox=self.bypass_approvals_and_sandbox,
        )

    def to_json(self) -> JsonObject:
        """Return the JSON-compatible service config shape."""
        return json_object(msgspec.to_builtins(self.to_wire()))


@dataclass(frozen=True)
class MockLmResponse:
    """One deterministic mock LM response with its charged token counts."""

    text: str
    input_tokens: int = 1
    output_tokens: int = 1


@dataclass(frozen=True)
class MockLmRuntimeConfig:
    """Deterministic LM provider config.

    `text` is the single-response convenience used when a request makes one LM
    call; `responses` overrides it with an ordered script (each executed LM call
    consumes the next response), needed when the host issues multiple LM calls
    such as optimize.run reflection.
    """

    text: str = "unused"
    input_tokens: int = 1
    output_tokens: int = 1
    responses: tuple[MockLmResponse, ...] | None = None

    def to_wire(self) -> MockLmRuntimeDocument:
        """Return the typed service config record."""
        responses = self.responses or (
            MockLmResponse(
                text=self.text,
                input_tokens=self.input_tokens,
                output_tokens=self.output_tokens,
            ),
        )
        return MockLmRuntimeDocument(
            responses=[
                MockLmResponseDocument(
                    text=response.text,
                    input_tokens=response.input_tokens,
                    output_tokens=response.output_tokens,
                )
                for response in responses
            ],
        )

    def to_json(self) -> JsonObject:
        """Return the JSON-compatible service config shape."""
        return json_object(msgspec.to_builtins(self.to_wire()))


@dataclass(frozen=True)
class OpenAiLmRuntimeConfig:
    """Live OpenAI Responses API provider config for `leaven/lm.complete`."""

    api_key_env: str = "OPENAI_API_KEY"
    base_url: str | None = None
    timeout_s: int | None = None
    max_retries: int | None = None

    def to_wire(self) -> OpenAiLmRuntimeDocument:
        """Return the typed service config record."""
        return OpenAiLmRuntimeDocument(
            api_key_env=self.api_key_env,
            base_url=self.base_url,
            timeout_s=self.timeout_s,
            max_retries=self.max_retries,
        )

    def to_json(self) -> JsonObject:
        """Return the JSON-compatible service config shape."""
        return json_object(msgspec.to_builtins(self.to_wire()))


@dataclass(frozen=True)
class MockRunnerStageConfig:
    """Deterministic stage runner config used for durable seam mechanics proofs."""

    text: str = "ok"
    summary: str = "mock runner output"

    def to_wire(self) -> MockRunnerStageDocument:
        """Return the typed service config record."""
        return MockRunnerStageDocument(text=self.text, summary=self.summary)

    def to_json(self) -> JsonObject:
        """Return the JSON-compatible service config shape."""
        return json_object(msgspec.to_builtins(self.to_wire()))


@dataclass(frozen=True)
class CommandRunnerStageConfig:
    """External JSON-RPC stage worker process config."""

    argv: tuple[str, ...]

    def to_wire(self) -> CommandRunnerStageDocument:
        """Return the typed service config record."""
        return CommandRunnerStageDocument(argv=list(self.argv))

    def to_json(self) -> JsonObject:
        """Return the JSON-compatible service config shape."""
        return json_object(msgspec.to_builtins(self.to_wire()))


@dataclass(frozen=True)
class LocalWorkspaceConfig:
    """Configured local workspace substrate for public-seam calls."""

    seed_files: dict[str, str] = field(default_factory=dict)
    parent: str | None = None

    def to_wire(self) -> LocalWorkspaceDocument:
        """Return the typed service config record."""
        return LocalWorkspaceDocument(
            seed_files=dict(self.seed_files),
            parent=self.parent if self.parent is not None else msgspec.UNSET,
        )

    def to_json(self) -> JsonObject:
        """Return the JSON-compatible service config shape."""
        return json_object(msgspec.to_builtins(self.to_wire()))


@dataclass(frozen=True)
class SeamServiceConfig:
    """Full private config document passed to `leaven seam serve --stdio`."""

    context: SeamExecutionContext
    capability: CapabilityDocument | None = None
    agent: CodexCliRuntimeConfig | None = None
    workspace: LocalWorkspaceConfig = field(default_factory=LocalWorkspaceConfig)
    lm: MockLmRuntimeConfig | OpenAiLmRuntimeConfig = field(default_factory=MockLmRuntimeConfig)
    stage: MockRunnerStageConfig | CommandRunnerStageConfig | None = None
    optimize_runs_root: str | None = None
    """Root directory under which `leaven/optimize.run` persists durable runs.

    When set, the host writes each run's checkpoint under
    `<optimize_runs_root>/<run_id>/`, so the client can read the durable run
    back. When unset, the host uses its Leaven-managed default run dir.
    """

    def to_wire(self) -> SeamServiceDocument:
        """Return the typed service config record."""
        return SeamServiceDocument(
            context=self.context.to_wire(),
            capability=self.capability,
            workspace=self.workspace.to_wire(),
            agent=self.agent.to_wire() if self.agent is not None else NoneProviderConfig(),
            lm=self.lm.to_wire(),
            stage=self.stage.to_wire() if self.stage is not None else NoneProviderConfig(),
            optimize_runs_root=(
                self.optimize_runs_root if self.optimize_runs_root is not None else msgspec.UNSET
            ),
        )

    def to_json(self) -> JsonObject:
        """Return the Rust service config JSON shape."""
        return config_to_json(self.to_wire())

    def to_json_bytes(self) -> bytes:
        """Return the encoded Rust service config JSON document."""
        return config_to_json_bytes(self.to_wire())


__all__ = [
    "CodexCliRuntimeConfig",
    "CommandRunnerStageConfig",
    "LocalWorkspaceConfig",
    "MockLmResponse",
    "MockLmRuntimeConfig",
    "MockRunnerStageConfig",
    "OpenAiLmRuntimeConfig",
    "SeamExecutionContext",
    "SeamServiceConfig",
    "config_to_json_bytes",
]
