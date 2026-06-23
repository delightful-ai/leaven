"""Leaven Harbor agent subclasses that inject an AgentKit into an agent.

These cover the cases that need agent-side behavior beyond a plain
``AgentConfig``:

- repo-scope (any agent): upload the staged kit's prompt file and skills into the
  task working directory before the agent runs.
- Codex user-scope: write the kit's system prompt to ``$CODEX_HOME/AGENTS.md``
  (read as global, workdir-independent appended context).

Claude Code user-scope needs no subclass: ``--append-system-prompt`` plus
``AgentConfig.skills`` is pure configuration.
"""

from pathlib import Path, PurePosixPath

from harbor.agents.installed.claude_code import ClaudeCode
from harbor.agents.installed.codex import Codex
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext

from leaven.x.harbor._kit import KIT_PROMPT_FILE, KIT_SKILLS_DIR


async def _upload_kit_tree(
    environment: BaseEnvironment,
    *,
    kit_dir: Path,
    workdir: str,
    prompt_file: str,
    skills_subdir: str,
) -> None:
    """Upload a staged kit into ``<workdir>`` as the agent's project surface."""
    root = PurePosixPath(workdir)
    prompt = kit_dir / KIT_PROMPT_FILE
    if prompt.is_file():
        await environment.upload_file(prompt, (root / prompt_file).as_posix())
    skills_root = kit_dir / KIT_SKILLS_DIR
    if not skills_root.is_dir():
        return
    for skill_file in sorted(skills_root.rglob("*")):
        if not skill_file.is_file():
            continue
        relative = skill_file.relative_to(skills_root)
        target = root / skills_subdir / PurePosixPath(relative.as_posix())
        await environment.upload_file(skill_file, target.as_posix())


class _LeavenKitMixin:
    """Shared kit-injection state parsed from ``AgentConfig.kwargs``."""

    def _init_kit(self, kwargs: dict[str, object]) -> None:
        self._placement = str(kwargs.pop("placement", "repo"))
        agent_kit_dir = kwargs.pop("agent_kit_dir", None)
        self._agent_kit_dir = Path(str(agent_kit_dir)) if agent_kit_dir else None
        self._workdir = str(kwargs.pop("workdir", "/app"))
        self._kit_prompt_file = str(kwargs.pop("kit_prompt_file", "AGENTS.md"))
        self._kit_skills_subdir = str(kwargs.pop("kit_skills_subdir", ".agents/skills"))

    async def _upload_repo_kit(self, environment: BaseEnvironment) -> None:
        if self._agent_kit_dir is None:
            return
        await _upload_kit_tree(
            environment,
            kit_dir=self._agent_kit_dir,
            workdir=self._workdir,
            prompt_file=self._kit_prompt_file,
            skills_subdir=self._kit_skills_subdir,
        )


class LeavenCodex(_LeavenKitMixin, Codex):
    """A Harbor Codex agent that injects a Leaven AgentKit (repo or user scope)."""

    def __init__(
        self,
        logs_dir: Path,
        *,
        model_name: str | None = None,
        extra_env: dict[str, str] | None = None,
        **kwargs: object,
    ) -> None:
        self._init_kit(kwargs)
        super().__init__(logs_dir, model_name=model_name, extra_env=extra_env, **kwargs)

    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        if self._placement == "repo":
            await self._upload_repo_kit(environment)
        elif self._placement == "user" and self._agent_kit_dir is not None:
            await self._write_codex_home_agents_md(environment)
        await super().run(instruction, environment, context)

    async def _write_codex_home_agents_md(self, environment: BaseEnvironment) -> None:
        """Place the kit prompt at ``$CODEX_HOME/AGENTS.md`` (global appended context)."""
        prompt = self._agent_kit_dir / KIT_PROMPT_FILE if self._agent_kit_dir else None
        if prompt is None or not prompt.is_file():
            return
        codex_home = self._REMOTE_CODEX_HOME.as_posix()
        await self.exec_as_agent(
            environment,
            command=f"mkdir -p {codex_home}",
            env={"CODEX_HOME": codex_home},
        )
        await environment.upload_file(prompt, f"{codex_home}/AGENTS.md")


class LeavenClaudeCode(_LeavenKitMixin, ClaudeCode):
    """A Harbor Claude Code agent that injects a repo-scope Leaven AgentKit.

    User-scope Claude Code needs no subclass (``--append-system-prompt`` plus
    ``AgentConfig.skills``); this subclass exists for ``placement="repo"``, which
    materializes the kit as ``<workdir>/CLAUDE.md`` and project skills.
    """

    def __init__(
        self,
        logs_dir: Path,
        *,
        model_name: str | None = None,
        extra_env: dict[str, str] | None = None,
        **kwargs: object,
    ) -> None:
        self._init_kit(kwargs)
        super().__init__(logs_dir, model_name=model_name, extra_env=extra_env, **kwargs)

    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        if self._placement == "repo":
            await self._upload_repo_kit(environment)
        await super().run(instruction, environment, context)


__all__ = ["LeavenClaudeCode", "LeavenCodex"]
