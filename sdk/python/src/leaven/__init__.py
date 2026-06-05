"""Leaven — Python authoring surface for the Leaven optimization framework.

The top-level import surface. Submodules under `lv.optimizers`, `lv.lm`,
`lv.agent`, `lv.workspace`, `lv.sandbox`, `lv.cases`, `lv.frontier`,
`lv.output`, `lv.scoring`, `lv.trust`, `lv.runs`, `lv.x` carry the
namespaced builders the spec names.

Governing spec: `docs/specs/leaven_python.md` in the parent repo.
"""

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
    run_status,
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
from .blob_ref import BlobRef

# ----- Composition + entry -------------------------------------------------
from .budget import Budget, budget
from .case import Case, CaseSet, CaseSplits, InputCaseView, ScoringCaseView
from .contexts import (
    EvaluatorContext,
    JudgeContext,
    ProposeContext,
    ReflectContext,
    RolloutContext,
    RubricContext,
)

# ----- Stage decorators + registration -------------------------------------
from .decorators import (
    evaluator,
    judge,
    proposer,
    reflector,
    register_stage,
    runner,
)
from .environment import Environment
from .optimize import OptimizeBuilder, optimize

# ----- Result + inspection -------------------------------------------------
from .result import Candidate, Optimized, ReplayResult, ReplayUnavailableError, RunSummary, Split
from .rubric import RewardValue, Rubric, reward
from .run_status import RunCostStatus, RunUsageStatus, UnsupportedRunFact
from .runtime import Cache, Runtime, runtime
from .score import Score
from .stages import Propose, Reflect, Rollout
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
# `runs`, `run_status`, `x`, `data_class`, `layouts`, `setup`) are imported above with
# `from . import ...` and stay.
# NOTE: `budget`, `optimize` are NOT in this list — they're
# public top-level callables that share names with their owning submodules.
# Deleting them would remove the function, not just the module attribute.
for _leaky in (
    "agent_instructions",
    "assessment",
    "blob_ref",
    "builders",
    "case",
    "contexts",
    "decorators",
    "evaluation_job",
    "environment",
    "evidence",
    "output_record",
    "proposal",
    "run_inspection",
    "result",
    "rubric",
    "score",
    "stage_payloads",
    "stages",
    "task",
):
    if _leaky in globals():
        del globals()[_leaky]
del _leaky

__all__ = [
    # records
    "AgentInstructions",
    "AgentRoles",
    "BlobRef",
    "Budget",
    "Cache",
    "Candidate",
    "Case",
    "CaseSet",
    "CaseSplits",
    "Environment",
    "EvaluatorContext",
    "InputCaseView",
    "JudgeContext",
    "OptimizeBuilder",
    "Optimized",
    "PromptArtifact",
    "Propose",
    "ProposeContext",
    "Reflect",
    "ReflectContext",
    "ReplayResult",
    "ReplayUnavailableError",
    "RewardValue",
    "Rollout",
    "RolloutContext",
    "Rubric",
    "RubricContext",
    "RunCostStatus",
    "RunSummary",
    "RunUsageStatus",
    "Runtime",
    "Score",
    "ScoringCaseView",
    "SkillBank",
    "SkillFile",
    "Split",
    "Task",
    "TrustProfile",
    "UnsupportedRunFact",
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
    # decorators + stage registration
    "evaluator",
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
    "reward",
    "roles",
    "run_status",
    "runner",
    "runs",
    "runtime",
    "sandbox",
    "scoring",
    "setup",
    "trust",
    "workspace",
    "x",
]
