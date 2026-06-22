"""Safe ATIF trajectory excerpts for optimizer-visible Harbor feedback."""

import json
from pathlib import Path

from leaven.x.harbor._types import HarborAdapterError


def trajectory_excerpt(path: str | Path | None, *, max_steps: int = 4, strict: bool = False) -> str:
    """Summarize recent agent-authored trajectory steps without task internals."""
    if path is None:
        return ""
    trajectory_path = Path(path)
    if not trajectory_path.is_file():
        return ""
    try:
        data = json.loads(trajectory_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        if strict:
            raise HarborAdapterError(f"invalid Harbor trajectory JSON: {exc}") from exc
        return ""
    if not isinstance(data, dict):
        return ""
    steps = data.get("steps", [])
    if not isinstance(steps, list):
        return ""
    agent_steps = [step for step in steps if isinstance(step, dict) and step.get("source") == "agent"]
    lines: list[str] = []
    for step in agent_steps[-max_steps:]:
        message = str(step.get("message") or "").strip().replace("\n", " ")
        names = _tool_names(step.get("tool_calls"))
        label = f"tool[{', '.join(names)}] " if names else ""
        if message or label:
            lines.append(f"- {label}{message[:240]}")
    return "\n".join(lines)


def _tool_names(tool_calls: object) -> list[str]:
    if not isinstance(tool_calls, list):
        return []
    return [
        call["function_name"]
        for call in tool_calls
        if isinstance(call, dict) and isinstance(call.get("function_name"), str)
    ]


__all__ = ["trajectory_excerpt"]
