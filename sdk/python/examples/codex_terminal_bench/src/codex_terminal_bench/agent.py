"""Terminal-Bench import-path shim for the generic Harbor Leaven Codex agent.

Harbor's built-in `Codex` agent installs `@openai/codex` in the task container
and runs `codex exec` in the task working directory. Codex reads `AGENTS.md`
from its working directory natively, so a Leaven agent kit (a system prompt plus
skill files) becomes the agent's authored instruction surface by uploading it
into the working directory before Codex runs.

The live Terminal-Bench proof keeps this module path stable for the Harbor
`AgentConfig.import_path`, but the implementation lives in `leaven.x.harbor`.
`LeavenCodex` defaults to repo-scope placement with `workdir="/app"`, which
preserves the original Terminal-Bench upload behavior (`AGENTS.md` plus
`.agents/skills/`) without a hardcoded constant in the example.
"""

from leaven.x.harbor import LeavenCodex

__all__ = ["LeavenCodex"]
