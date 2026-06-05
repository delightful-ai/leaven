"""Data-class constants — the v1 security primitive for visibility/taint labels.

See `docs/specs/leaven_python.md` ("What is preserved") and
`docs/specs/public-seam-v1/schemas/common.schema.json` (DataClass enum).

Data classes propagate monotonically. A call whose accumulated input classes
intersect any forbidden class is denied before execution. The locked seam
owns enforcement; the Python side declares.

These constants are convenience labels; the wire format is the string itself.
Use either the constant or the string literal — both are accepted.
"""

from typing import Final

# Case-derived classes
PUBLIC: Final = "public"
CASE_INPUT: Final = "case.input"
CASE_TARGET: Final = "case.target"
CASE_METADATA: Final = "case.metadata"

# Candidate-derived classes
CANDIDATE_OUTPUT: Final = "candidate.output"
CANDIDATE_ARTIFACT: Final = "candidate.artifact"

# Workspace-derived classes
WORKSPACE_FILE: Final = "workspace.file"
WORKSPACE_DIFF: Final = "workspace.diff"
WORKSPACE_SECRET: Final = "workspace.secret"

# Provenance/visibility classes
OPTIMIZER_VISIBLE: Final = "optimizer.visible"
EVALUATOR_PRIVATE: Final = "evaluator.private"
SCORER_PRIVATE: Final = "scorer.private"
TRANSCRIPT_RAW: Final = "transcript.raw"

# Extension namespace marker — `x.<adapter>.*` data classes follow the same
# convention as the `x.*` schema namespace.
EXTENSION_PREFIX: Final = "x."

__all__ = [
    "CANDIDATE_ARTIFACT",
    "CANDIDATE_OUTPUT",
    "CASE_INPUT",
    "CASE_METADATA",
    "CASE_TARGET",
    "EVALUATOR_PRIVATE",
    "EXTENSION_PREFIX",
    "OPTIMIZER_VISIBLE",
    "PUBLIC",
    "SCORER_PRIVATE",
    "TRANSCRIPT_RAW",
    "WORKSPACE_DIFF",
    "WORKSPACE_FILE",
    "WORKSPACE_SECRET",
]
