"""Leaven — the Python authoring surface for the Leaven optimization framework.

`lv.*` is product nouns ONLY. Advanced authoring types live in `lv.adapters.*`;
generated public-seam schema records live in `lv.wire.*`; private engine helpers
live in `lv._engine.*`.

The core law:

    evolution = artifact x task x stages x optimizer x runtime

Entry point:

    result = await lv.evolve(
        artifact=artifact, task=task, stages=stages,
        optimizer=optimizer, runtime=runtime,
    ).run()

Governing spec: `docs/specs/leaven_python.md`.
"""

from __future__ import annotations

# Namespaces (product nouns that are sub-namespaces)
from . import (
    agent,
    artifacts,
    assets,
    cases,
    gepa,
    layouts,
    lm,
    optimizers,
    output,
    runs,
    sandbox,
    scorers,
    setup,
    workspace,
    x,
)
from .adapters.reflective import ReflectiveBatch
from .budget import budget
from .case import Case
from .context import Context
from .decorators import evaluator, proposer, reflector, runner, scorer, serve
from .evolve import evolve
from .propose import Proposal, Propose
from .reflect import Reflect
from .rollout import Rollout, RolloutResult
from .runtime import Runtime, runtime
from .score import Score, Scorer
from .stages import Stages
from .task import Task
from .trust import trust

__version__ = "0.1.0-alpha.0"

__all__ = [
    "Case",
    "Context",
    "Proposal",
    "Propose",
    "Reflect",
    "ReflectiveBatch",
    "Rollout",
    "RolloutResult",
    "Runtime",
    "Score",
    "Scorer",
    "Stages",
    "Task",
    "__version__",
    "agent",
    "artifacts",
    "assets",
    "budget",
    "cases",
    "evaluator",
    "evolve",
    "gepa",
    "layouts",
    "lm",
    "optimizers",
    "output",
    "proposer",
    "reflector",
    "runner",
    "runs",
    "runtime",
    "sandbox",
    "scorer",
    "scorers",
    "serve",
    "setup",
    "trust",
    "workspace",
    "x",
]
