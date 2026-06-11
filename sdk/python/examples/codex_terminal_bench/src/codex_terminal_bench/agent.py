"""`LeavenCodex` — a Harbor Codex agent that mounts a Leaven agent kit.

Harbor's built-in `Codex` agent installs `@openai/codex` in the task container
and runs `codex exec` in the task working directory. Codex reads `AGENTS.md`
from its working directory natively, so a Leaven agent kit (a system prompt plus
skill files) becomes the agent's authored instruction surface by uploading it
into the working directory before Codex runs.

This subclass uploads the materialized kit (the system prompt as `AGENTS.md`,
plus each skill file under the working directory's skills subtree) into the task
`WORKDIR` before delegating to the stock `Codex.run`. The kit directory is
passed through Harbor's `AgentConfig.kwargs` as `agent_kit_dir`; the regex-log
task's `WORKDIR` is `/app`, so the kit lands at `/app/AGENTS.md` and
`/app/.agents/skills/<path>`.

The agent is selected through `AgentConfig.import_path` so the optimizer's
rollout can run it without registering a new Harbor agent name.
"""

from pathlib import Path, PurePosixPath

from harbor.agents.installed.codex import Codex
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext

# The terminal-bench-2 regex-log task working directory. Codex reads AGENTS.md
# from this directory natively, and the agent writes its answer under it.
DEFAULT_WORKDIR = "/app"
# Skills subtree Codex consults, relative to the working directory.
SKILLS_SUBDIR = ".agents/skills"


class LeavenCodex(Codex):
    """A Harbor Codex agent that mounts a Leaven agent kit into the WORKDIR."""

    def __init__(
        self,
        logs_dir: Path,
        *,
        model_name: str | None = None,
        extra_env: dict[str, str] | None = None,
        agent_kit_dir: str | None = None,
        workdir: str = DEFAULT_WORKDIR,
        **kwargs: str | None,
    ) -> None:
        # Harbor's `AgentFactory.create_agent_from_config` instantiates an
        # import-path agent with `logs_dir`, `model_name`, `extra_env`, and the
        # config kwargs, all by keyword. The forwarded `model_name`/`extra_env`
        # are named explicitly so the parent's typed initializer is satisfied;
        # remaining keys (e.g. `version`, `prompt_template_path`) ride `**kwargs`.
        self._agent_kit_dir = Path(agent_kit_dir) if agent_kit_dir else None
        self._workdir = PurePosixPath(workdir)
        super().__init__(logs_dir, model_name=model_name, extra_env=extra_env, **kwargs)

    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        """Upload the agent kit into the WORKDIR, then run stock Codex."""
        if self._agent_kit_dir is not None:
            await self._upload_agent_kit(environment)
        await super().run(instruction, environment, context)

    async def _upload_agent_kit(self, environment: BaseEnvironment) -> None:
        kit_dir = self._agent_kit_dir
        if kit_dir is None:
            return
        agents_md = kit_dir / "AGENTS.md"
        if agents_md.is_file():
            await environment.upload_file(
                agents_md,
                (self._workdir / "AGENTS.md").as_posix(),
            )
        skills_root = kit_dir / "skills"
        if skills_root.is_dir():
            for skill_file in sorted(skills_root.rglob("*")):
                if not skill_file.is_file():
                    continue
                relative = skill_file.relative_to(skills_root)
                target = self._workdir / SKILLS_SUBDIR / PurePosixPath(relative.as_posix())
                await environment.upload_file(skill_file, target.as_posix())


__all__ = ["DEFAULT_WORKDIR", "SKILLS_SUBDIR", "LeavenCodex"]
