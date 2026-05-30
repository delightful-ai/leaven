"""Reflect — `lv.Reflect`.

Reflect turns a feedback-annotated minibatch of scored attempts into a
free-form diagnosis. It is OPTIONAL: when omitted, reflection is fused into
propose (GEPA installs the default). It is EITHER a function you write OR the
declarative built-in `Reflect.agent`.

The reflect function signature is `(batch: ReflectiveBatch, cx: Context) ->
str | None`. There is NO `Critique` type — reflection output is free-form `str`
(or `None`). The diagnosis is just text; Propose consumes it as `reflection:
str | None`. Reflect does NOT produce a candidate change — that is Propose's
job.

Build-once-pass-down: the optimizer constructs the reflective batch once,
target-safe, and hands `reflect` a finished batch; the reflect function does NOT
query run history to assemble its own evidence.

Governing spec: `docs/specs/leaven_python.md` — Reflect.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .agent.config import AgentConfig
    from .layouts import Layout

__all__ = ["Reflect"]


class Reflect:
    """Namespace of declarative engine-mediated reflection built-ins."""

    @staticmethod
    def agent(
        agent: AgentConfig,
        *,
        layout: Layout | None = None,
        instructions: str | None = None,
    ) -> Reflect:
        """Engine-mediated reflection. Mirrors `Rollout.agent` minus `output`
        (reflection has no parsed output contract); the engine returns the
        reflector's free-form diagnosis text. Spec — Reflect."""
        raise NotImplementedError("see leaven_python.md — Reflect.agent")
