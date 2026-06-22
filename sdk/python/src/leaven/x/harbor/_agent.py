"""Harbor Codex agent subclass that uploads a Leaven AgentKit into WORKDIR."""

from pathlib import Path, PurePosixPath

from harbor.agents.installed.codex import Codex
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext

DEFAULT_WORKDIR = "/app"
SKILLS_SUBDIR = ".agents/skills"


class LeavenCodex(Codex):
    """A Harbor Codex agent that mounts a Leaven agent kit into the workdir."""

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
        self._agent_kit_dir = Path(agent_kit_dir) if agent_kit_dir else None
        self._workdir = PurePosixPath(workdir)
        super().__init__(logs_dir, model_name=model_name, extra_env=extra_env, **kwargs)

    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        if self._agent_kit_dir is not None:
            await self._upload_agent_kit(environment)
        await super().run(instruction, environment, context)

    async def _upload_agent_kit(self, environment: BaseEnvironment) -> None:
        kit_dir = self._agent_kit_dir
        if kit_dir is None:
            return
        agents_md = kit_dir / "AGENTS.md"
        if agents_md.is_file():
            await environment.upload_file(agents_md, (self._workdir / "AGENTS.md").as_posix())
        skills_root = kit_dir / "skills"
        if not skills_root.is_dir():
            return
        for skill_file in sorted(skills_root.rglob("*")):
            if not skill_file.is_file():
                continue
            relative = skill_file.relative_to(skills_root)
            target = self._workdir / SKILLS_SUBDIR / PurePosixPath(relative.as_posix())
            await environment.upload_file(skill_file, target.as_posix())


__all__ = ["DEFAULT_WORKDIR", "SKILLS_SUBDIR", "LeavenCodex"]
