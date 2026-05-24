"""Data-class constants — the v1 security primitive for visibility/taint labels.

See `docs/specs/leaven_python.md` ("What is preserved") and
`docs/specs/public-seam-v1/schemas/common.schema.json` (DataClass enum).

Data classes propagate monotonically. A call whose accumulated input classes
intersect any forbidden class is denied before execution. The locked seam
owns enforcement; the Python side declares.

These constants are convenience labels; the wire format is the string itself.
Use either the constant or the string literal — both are accepted.
"""

from __future__ import annotations

from typing import Final

# Case-derived classes
CASE_INPUT: Final = "case.input"
CASE_TARGET: Final = "case.target"
CASE_METADATA: Final = "case.metadata"

# Candidate-derived classes
CANDIDATE_OUTPUT: Final = "candidate.output"
ARTIFACT_OUTPUT: Final = "artifact.output"

# Workspace-derived classes
WORKSPACE_FILE: Final = "workspace.file"
WORKSPACE_DIFF: Final = "workspace.diff"
WORKSPACE_SECRET: Final = "workspace.secret"

# Provenance/visibility classes
OPTIMIZER_VISIBLE: Final = "optimizer.visible"
EVALUATOR_PRIVATE: Final = "evaluator.private"
TRACE_ONLY: Final = "trace.only"

# Extension namespace marker — `x.<adapter>.*` data classes follow the same
# convention as the `x.*` schema namespace; e.g. `x.dspy.completion`.
EXTENSION_PREFIX: Final = "x."

__all__ = [
    "ARTIFACT_OUTPUT",
    "CANDIDATE_OUTPUT",
    "CASE_INPUT",
    "CASE_METADATA",
    "CASE_TARGET",
    "EVALUATOR_PRIVATE",
    "EXTENSION_PREFIX",
    "OPTIMIZER_VISIBLE",
    "TRACE_ONLY",
    "WORKSPACE_DIFF",
    "WORKSPACE_FILE",
    "WORKSPACE_SECRET",
]
