"""Private run-status facts observed by the durable-seam optimize route."""

from __future__ import annotations

from typing import Any

from ..agent.codex import CodexAgent
from ..run_status import UnsupportedRunFact
from ..runtime import Runtime


def unsupported_facts_for_runtime(runtime: Runtime) -> tuple[UnsupportedRunFact, ...]:
    """Return public unsupported facts caused by private seam dependencies."""
    facts: list[UnsupportedRunFact] = [
        UnsupportedRunFact(
            surface="run.inspection",
            dependency="python_seam_optimize",
            reason="blob_readback_not_implemented",
            detail=(
                "this optimize mechanics path persists blob ref metadata but does not yet "
                "provide blob-content fetch for lv.runs.open/readback"
            ),
        )
    ]
    agent = first_agent(runtime.agent)
    if isinstance(agent, CodexAgent) and agent.transport == "cli":
        facts.extend(
            [
                UnsupportedRunFact(
                    surface="run.cost",
                    dependency="codex_cli",
                    reason="provider_cost_not_reported",
                    detail=(
                        "Codex CLI agent callbacks can spend live tokens, but this seam "
                        "slice does not receive an authoritative total_cost_usd"
                    ),
                ),
                UnsupportedRunFact(
                    surface="run.usage",
                    dependency="codex_cli",
                    reason="provider_usage_not_reported",
                    detail=(
                        "Codex CLI agent callbacks can use LM tokens, but this seam "
                        "slice does not receive authoritative token totals"
                    ),
                ),
            ]
        )
    return tuple(facts)


def first_agent(value: Any) -> Any | None:
    """Return the first configured agent without assigning provider semantics."""
    if value is None:
        return None
    if isinstance(value, list):
        return value[0] if value else None
    if isinstance(value, dict):
        return next(iter(value.values())) if value else None
    return value


__all__ = ["first_agent", "unsupported_facts_for_runtime"]
