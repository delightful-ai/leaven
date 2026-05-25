"""Leaven — Python authoring surface for the Leaven optimization framework.

The top-level import surface. Submodules under `lv.optimizers`, `lv.lm`,
`lv.agent`, `lv.workspace`, `lv.sandbox`, `lv.cases`, `lv.frontier`,
`lv.output`, `lv.scoring`, `lv.trust`, `lv.runs`, `lv.x` carry the
namespaced builders the spec names.

Governing spec: `docs/specs/leaven_python.md` in the parent repo.
"""

from __future__ import annotations

# Versioning
__version__ = "0.1.0-alpha.0"

# ----- Records (the wire-shaped types users construct) ----------------------
# Namespace re-exports (submodules) — these stay as `lv.<name>.*`
from . import (
    agent,
    artifacts,
    cases,
    data_class,
    frontier,
    layouts,
    lm,
    optimizers,
    output,
    runs,
    sandbox,
    scoring,
    setup,
    trust,
    workspace,
    x,
)
from .agent_instructions import AgentInstructions, AgentRoles

# ----- Built-in artifacts ---------------------------------------------------
from .artifacts import PromptArtifact, SkillBank
from .artifacts.skill_bank import SkillFile

# ----- Composition + entry -------------------------------------------------
from .budget import Budget, budget
from .case import Case, CaseSet, CaseSplits

# ----- Stage decorators + registration -------------------------------------
from .decorators import (
    evaluator,
    judge,
    proposer,
    reflector,
    register_stage,
    runner,
    scorer,
    serve_stage,
)
from .environment import Cache, Environment, environment
from .evolution import EvolutionBuilder, evolve
from .optimize import OptimizeBuilder, optimize

# ----- Result + inspection -------------------------------------------------
from .result import Candidate, Optimized, ReplayResult, RunSummary, Split
from .runtime import Runtime, runtime
from .score import Score
from .stages import Evaluate, Propose, Reflect, Rollout, ScoreStage, Stages
from .task import Task

# ----- Trust profile constants ---------------------------------------------
from .trust import TrustProfile

# ----- Top-level convenience re-exports from x.dspy ------------------------
# (the locked spec example uses `lv.dspy_context(...)` and `lv.dspy_acall(...)` inline)
from .x.dspy.context import dspy_call_context, dspy_context
from .x.dspy.invoke import dspy_acall

# Roles convention alias (used as `lv.roles.EXECUTOR` in evaluator code).
roles = AgentRoles

# ----- Hide leak-y submodule names -----------------------------------------
# Submodules whose public types we re-export at the top level (e.g. `case`,
# `assessment`, `decorators`, `evaluation_job`, ...) get attached to `leaven`
# as a side-effect of `from .X import Y`. We delete them so `dir(leaven)`
# only shows the deliberate surface — the convention in AGENTS.md is that
# users access these types as `lv.Case`, `lv.Assessment`, not `lv.case.Case`.
# Submodules INTENDED as namespaces (`agent`, `artifacts`, `lm`, `workspace`,
# `sandbox`, `cases`, `optimizers`, `frontier`, `output`, `scoring`, `trust`,
# `runs`, `x`, `data_class`, `layouts`, `setup`) are imported above with
# `from . import ...` and stay.
# NOTE: `budget`, `environment`, `optimize` are NOT in this list — they're
# public top-level callables that share names with their owning submodules.
# Deleting them would remove the function, not just the module attribute.
for _leaky in (
    "agent_instructions", "assessment", "builders",
    "case", "context", "decorators", "evaluation_job",
    "evidence", "evolution", "output_record", "proposal", "result",
    "score", "stage_payloads", "stages", "task",
):
    if _leaky in globals():
        del globals()[_leaky]
del _leaky

__all__ = [
    # records
    "AgentInstructions",
    "AgentRoles",
    "Budget",
    "Cache",
    "Candidate",
    "Case",
    "CaseSet",
    "CaseSplits",
    "Environment",
    "Evaluate",
    "EvolutionBuilder",
    "OptimizeBuilder",
    "Optimized",
    "PromptArtifact",
    "Propose",
    "Reflect",
    "ReplayResult",
    "Rollout",
    "RunSummary",
    "Runtime",
    "Score",
    "ScoreStage",
    "SkillBank",
    "SkillFile",
    "Split",
    "Stages",
    "Task",
    "TrustProfile",
    # versioning
    "__version__",
    # namespaces
    "agent",
    "artifacts",
    # entry + composition
    "budget",
    "cases",
    "data_class",
    # convenience re-exports from x.dspy
    "dspy_acall",
    "dspy_call_context",
    "dspy_context",
    "environment",
    # decorators + stage registration
    "evaluator",
    "evolve",
    "frontier",
    "judge",
    "layouts",
    "lm",
    "optimize",
    "optimizers",
    "output",
    "proposer",
    "reflector",
    "register_stage",
    "roles",
    "runner",
    "runs",
    "runtime",
    "sandbox",
    "scorer",
    "scoring",
    "serve_stage",
    "setup",
    "trust",
    "workspace",
    "x",
]
