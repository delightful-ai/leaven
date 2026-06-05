"""Private run-status facts observed by the durable-seam optimize route."""

from ..agent.codex import CodexAgent
from ..agent.config import AgentConfig
from ..run_status import UnsupportedRunFact
from ..runtime import Runtime


def unsupported_facts_for_runtime(runtime: Runtime) -> tuple[UnsupportedRunFact, ...]:
    """Return public unsupported facts caused by private seam dependencies."""
    facts: list[UnsupportedRunFact] = []
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


def first_agent(
    agent: AgentConfig | list[AgentConfig] | dict[str, AgentConfig] | None,
) -> AgentConfig | None:
    """Return the first configured agent without assigning provider semantics."""
    if agent is None:
        return None
    if isinstance(agent, list):
        return agent[0] if agent else None
    if isinstance(agent, dict):
        return next(iter(agent.values())) if agent else None
    return agent


__all__ = ["first_agent", "unsupported_facts_for_runtime"]
