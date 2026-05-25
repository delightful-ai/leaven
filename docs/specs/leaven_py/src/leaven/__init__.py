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
    cases,
    data_class,
    frontier,
    lm,
    optimizers,
    output,
    runs,
    sandbox,
    scoring,
    trust,
    workspace,
    x,
)

# ----- Handles (engine-owned resources) ------------------------------------
from ._handles import CandidateHandle, WorkspaceHandle, WorkspaceLifetime, WorkspaceSurface

# ----- Receipts (opaque handles) -------------------------------------------
from ._receipts import CallReceipt, QueryReceipt, WriteReceipt
from .agent_instructions import AgentInstructions, AgentRoles

# ----- Built-in artifacts ---------------------------------------------------
from .artifacts import PromptArtifact, SkillBank
from .artifacts.skill_bank import SkillFile
from .assessment import Assessment, AssessmentWrite, Replayability

# ----- Composition + entry -------------------------------------------------
from .budget import Budget, budget
from .case import Case, CaseSet, CaseSplits

# ----- Context objects (the `cx` parameter) --------------------------------
from .context import EvalContext, RunContext, StageContext

# ----- Stage decorators + registration -------------------------------------
from .decorators import (
    RegisteredStage,
    StageRole,
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
from .evaluation_job import EvaluationItem, EvaluationJob, Granularity
from .evidence import EvidenceEnvelope, EvidencePrivate, EvidencePublic
from .optimize import OptimizeBuilder, optimize
from .output_record import OutputRecord, Visibility
from .proposal import ProposalBatch, ProposalEffect

# ----- Result + inspection -------------------------------------------------
from .result import Candidate, Optimized, ReplayResult, RunSummary, Split
from .score import Score
from .stage_payloads import (
    JudgeRequest,
    ProposeRequest,
    ReflectExample,
    ReflectionResult,
    ReflectRequest,
    StageSourceRef,
)

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
# Submodules INTENDED as namespaces (`agent`, `lm`, `workspace`, `sandbox`,
# `cases`, `optimizers`, `frontier`, `output`, `scoring`, `trust`, `runs`,
# `x`, `data_class`) are imported above with `from . import ...` and stay.
# NOTE: `budget`, `environment`, `optimize` are NOT in this list — they're
# public top-level callables that share names with their owning submodules.
# Deleting them would remove the function, not just the module attribute.
for _leaky in (
    "agent_instructions", "artifacts", "assessment", "builders",
    "case", "context", "decorators", "evaluation_job",
    "evidence", "output_record", "proposal", "result",
    "score", "stage_payloads",
):
    if _leaky in globals():
        del globals()[_leaky]
del _leaky

__all__ = [
    # records
    "AgentInstructions",
    "AgentRoles",
    "Assessment",
    "AssessmentWrite",
    "Budget",
    "Cache",
    "CallReceipt",
    "Candidate",
    "CandidateHandle",
    "Case",
    "CaseSet",
    "CaseSplits",
    "Environment",
    "EvalContext",
    "EvaluationItem",
    "EvaluationJob",
    "EvidenceEnvelope",
    "EvidencePrivate",
    "EvidencePublic",
    "Granularity",
    "JudgeRequest",
    "OptimizeBuilder",
    "Optimized",
    "OutputRecord",
    "PromptArtifact",
    "ProposalBatch",
    "ProposalEffect",
    "ProposeRequest",
    "QueryReceipt",
    "ReflectExample",
    "ReflectRequest",
    "ReflectionResult",
    "RegisteredStage",
    "ReplayResult",
    "Replayability",
    "RunContext",
    "RunSummary",
    "Score",
    "SkillBank",
    "SkillFile",
    "Split",
    "StageContext",
    "StageRole",
    "StageSourceRef",
    "TrustProfile",
    "Visibility",
    "WorkspaceHandle",
    "WorkspaceLifetime",
    "WorkspaceSurface",
    "WriteReceipt",
    # versioning
    "__version__",
    # namespaces
    "agent",
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
    "frontier",
    "judge",
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
    "sandbox",
    "scorer",
    "scoring",
    "serve_stage",
    "trust",
    "workspace",
    "x",
]
