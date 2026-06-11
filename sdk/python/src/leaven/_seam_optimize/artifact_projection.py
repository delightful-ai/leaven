"""Artifact-type projection for the optimize-run path.

`lv.optimize(...)` can optimize different artifact types (a `PromptArtifact`
template or an `AgentKitArtifact` agent kit). Each artifact type has its own
wire `artifact_type`, seed-artifact lowering, host reflection path, runner-stage
candidate payload key, and result-record projection. This module is the single
owner of that per-artifact-type knowledge so the driver, the worker runner, and
the result facade do not each hard-code one artifact type.

The reflection kind is fixed by the artifact type, mirroring the host's
artifact-type x reflection-kind matrix: a `prompt` seed reflects with an LM, an
`agent_kit` seed reflects agentically through a configured agent runtime.
"""

from dataclasses import dataclass
from typing import Literal

from .._seam import ArtifactRecord
from ..artifacts.agent_kit import (
    AGENT_KIT_ARTIFACT_SCHEMA,
    AGENT_KIT_ARTIFACT_TYPE,
    AgentKitArtifact,
)
from ..artifacts.prompt import PromptArtifact
from ..json_value import JsonObject

PROMPT_ARTIFACT_TYPE = "prompt"
PROMPT_ARTIFACT_SCHEMA = "fp_schema_sha256_prompt"

# The runner-stage candidate payload key the host projects each candidate under,
# read by the worker runner. The prompt path carries a `candidate_template`
# string; the agent-kit path carries a `candidate_agent_kit` flat wire artifact.
PROMPT_CANDIDATE_KEY = "candidate_template"
AGENT_KIT_CANDIDATE_KEY = "candidate_agent_kit"

type OptimizeSeed = PromptArtifact | AgentKitArtifact
type ReflectionKind = Literal["lm", "agentic"]


@dataclass(frozen=True)
class SeedProjection:
    """A seed artifact lowered into its wire record plus its reflection kind."""

    artifact: ArtifactRecord
    reflection_kind: ReflectionKind


def project_seed(seed: OptimizeSeed) -> SeedProjection:
    """Lower a seed artifact into its wire record and host reflection kind."""
    if isinstance(seed, PromptArtifact):
        return SeedProjection(
            artifact=ArtifactRecord(
                artifact_type=PROMPT_ARTIFACT_TYPE,
                artifact_schema=PROMPT_ARTIFACT_SCHEMA,
                artifact={"template": seed.template},
            ),
            reflection_kind="lm",
        )
    if isinstance(seed, AgentKitArtifact):
        return SeedProjection(
            artifact=ArtifactRecord(
                artifact_type=AGENT_KIT_ARTIFACT_TYPE,
                artifact_schema=AGENT_KIT_ARTIFACT_SCHEMA,
                artifact=seed.to_wire_artifact(),
            ),
            reflection_kind="agentic",
        )
    raise TypeError(
        f"lv.optimize optimizes a PromptArtifact or AgentKitArtifact seed; "
        f"got {type(seed).__name__}"
    )


def artifact_from_record(artifact_type: str, artifact: JsonObject, candidate_id: str) -> OptimizeSeed:
    """Project a result/candidate wire artifact record into a typed artifact."""
    if artifact_type == PROMPT_ARTIFACT_TYPE:
        if "template" not in artifact:
            raise TypeError(
                f"optimize.run candidate {candidate_id!r} prompt artifact has no template"
            )
        template = artifact["template"]
        if not isinstance(template, str):
            raise TypeError(
                f"optimize.run candidate {candidate_id!r} prompt artifact has no string template"
            )
        return PromptArtifact(template=template, candidate_id=candidate_id)
    if artifact_type == AGENT_KIT_ARTIFACT_TYPE:
        return AgentKitArtifact.from_wire_artifact(artifact, candidate_id=candidate_id)
    raise TypeError(
        f"optimize.run candidate {candidate_id!r} carries unknown artifact_type {artifact_type!r}"
    )


__all__ = [
    "AGENT_KIT_ARTIFACT_SCHEMA",
    "AGENT_KIT_ARTIFACT_TYPE",
    "AGENT_KIT_CANDIDATE_KEY",
    "PROMPT_ARTIFACT_SCHEMA",
    "PROMPT_ARTIFACT_TYPE",
    "PROMPT_CANDIDATE_KEY",
    "OptimizeSeed",
    "ReflectionKind",
    "SeedProjection",
    "artifact_from_record",
    "project_seed",
]
