"""Propose — `lv.Propose`, `lv.Proposal`.

Propose turns a parent artifact + the feedback-annotated batch + the reflector's
free-form diagnosis into a candidate change (`Proposal`). EITHER a function you
write OR the declarative built-in `Propose.agent_edit`.

The propose function signature is `(parent, batch: ReflectiveBatch, reflection:
str | None, cx: Context) -> Proposal`. `reflection` is the reflector's free-form
text (or `None` when reflect is omitted/fused). `parent` is typed as the
artifact type.

`Propose.agent_edit` materializes the parent under `target/current/`, runs the
agent as editor with the diagnosis attached, and reads the workspace back as a
typed artifact-native change; edits outside the artifact's `mutable=` surface
are rejected on readback.

Governing spec: `docs/specs/leaven_python.md` — Propose.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from pydantic import BaseModel, ConfigDict

from .wire.proposal import ProposalEffect

if TYPE_CHECKING:
    from .agent.config import AgentConfig
    from .layouts import Layout

__all__ = ["Proposal", "Propose"]


class Proposal(BaseModel):
    """The proposer's typed proposal. `change` is the artifact-native change-set,
    opaque at the product layer. `effect` reuses `wire.ProposalEffect`."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    instructions: str | None = None
    change: object | None = None
    rationale: str = ""
    effect: ProposalEffect = ProposalEffect.change


class Propose:
    """Namespace of declarative engine-mediated artifact-edit built-ins."""

    @staticmethod
    def agent_edit(
        agent: AgentConfig,
        *,
        layout: Layout | None = None,
        instructions: str | None = None,
    ) -> Propose:
        """Engine-mediated artifact edit + typed readback (the MVP default).
        Spec line 715."""
        raise NotImplementedError("see leaven_python.md — Propose.agent_edit")
