"""Registry of Harbor agents Leaven can drive for AgentKit rollouts.

Each `HarborAgentAdapter` knows how to project a staged AgentKit (system prompt +
skills) into a Harbor `AgentConfig` for a chosen `placement`:

- ``placement="user"`` is workdir-independent for Codex: the adapter writes the
  system prompt to ``$CODEX_HOME/AGENTS.md`` (read as global appended context)
  and takes skills via ``AgentConfig.skills`` (``$HOME/.agents/skills``).
- ``placement="repo"`` materializes the kit into the task ``workdir``: Codex reads
  ``<workdir>/AGENTS.md`` + ``<workdir>/.agents/skills`` (scanned cwd→repo root),
  Claude Code reads ``<workdir>/CLAUDE.md`` + ``<workdir>/.claude/skills/<n>/SKILL.md``
  (portable AgentKit skill files are projected into Claude skill packages).

Claude Code user placement is refused until Harbor quotes
``--append-system-prompt`` values safely; otherwise multiword prompts are split
by the shell before the real task instruction.

Harbor itself is imported lazily inside :meth:`HarborAgentAdapter.agent_config`,
so importing this module (and ``leaven``) never requires Harbor.
"""

from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING

from leaven.x.harbor._kit import staged_skill_paths
from leaven.x.harbor._types import HarborAdapterError

if TYPE_CHECKING:
    from harbor.models.trial.config import AgentConfig

CODEX_IMPORT_PATH = "leaven.x.harbor:LeavenCodex"
CLAUDE_CODE_IMPORT_PATH = "leaven.x.harbor:LeavenClaudeCode"


@dataclass(frozen=True)
class HarborAgentAdapter:
    """How one Harbor agent receives a Leaven AgentKit."""

    key: str
    harbor_name: str
    leaven_import_path: str
    repo_prompt_file: str
    repo_skills_subdir: str
    repo_skills_layout: str  # "portable" | "claude_packages"
    default_model: str
    api_key_env: str
    user_prompt_mode: str  # "unsupported_append_flag" | "codex_home"
    default_placement: str  # "user" | "repo"

    def agent_config(
        self,
        *,
        model: str,
        placement: str,
        workdir: str,
        staging_dir: Path,
        api_key: str,
        agent_env: dict[str, str] | None = None,
    ) -> "AgentConfig":
        """Build the Harbor ``AgentConfig`` that injects the staged kit."""
        from harbor.models.trial.config import AgentConfig  # noqa: PLC0415

        env = {self.api_key_env: api_key} if api_key else {}
        if agent_env:
            env.update(agent_env)

        if placement == "repo":
            return AgentConfig(
                import_path=self.leaven_import_path,
                model_name=model,
                kwargs={
                    "placement": "repo",
                    "agent_kit_dir": str(staging_dir),
                    "workdir": workdir,
                    "kit_prompt_file": self.repo_prompt_file,
                    "kit_skills_subdir": self.repo_skills_subdir,
                    "kit_skills_layout": self.repo_skills_layout,
                },
                env=env,
            )

        skills = staged_skill_paths(staging_dir)
        if self.user_prompt_mode == "unsupported_append_flag":
            raise HarborAdapterError(
                f"{self.key} user placement is disabled: Harbor renders "
                "--append-system-prompt without shell quoting, so multiword kit prompts "
                "can replace the task instruction. Use placement='repo'."
            )

        # Codex user scope: the subclass writes $CODEX_HOME/AGENTS.md.
        return AgentConfig(
            import_path=self.leaven_import_path,
            model_name=model,
            skills=skills,
            kwargs={"placement": "user", "agent_kit_dir": str(staging_dir)},
            env=env,
        )


AGENTS: dict[str, HarborAgentAdapter] = {
    "codex": HarborAgentAdapter(
        key="codex",
        harbor_name="codex",
        leaven_import_path=CODEX_IMPORT_PATH,
        repo_prompt_file="AGENTS.md",
        repo_skills_subdir=".agents/skills",
        repo_skills_layout="portable",
        default_model="openai/gpt-5.4-mini",
        api_key_env="OPENAI_API_KEY",
        user_prompt_mode="codex_home",
        default_placement="repo",
    ),
    "claude-code": HarborAgentAdapter(
        key="claude-code",
        harbor_name="claude-code",
        leaven_import_path=CLAUDE_CODE_IMPORT_PATH,
        repo_prompt_file="CLAUDE.md",
        repo_skills_subdir=".claude/skills",
        repo_skills_layout="claude_packages",
        default_model="anthropic/claude-sonnet-4-6",
        api_key_env="ANTHROPIC_API_KEY",
        user_prompt_mode="unsupported_append_flag",
        default_placement="repo",
    ),
}


def resolve(agent: str) -> HarborAgentAdapter:
    """Resolve a registered Harbor agent key into its adapter."""
    if agent in AGENTS:
        return AGENTS[agent]
    raise HarborAdapterError(
        f"unknown Harbor agent {agent!r}; supported agents are "
        f"{sorted(AGENTS)}. Register a HarborAgentAdapter in "
        "leaven.x.harbor.agents.AGENTS to add another."
    )


__all__ = [
    "AGENTS",
    "CLAUDE_CODE_IMPORT_PATH",
    "CODEX_IMPORT_PATH",
    "HarborAgentAdapter",
    "resolve",
]
