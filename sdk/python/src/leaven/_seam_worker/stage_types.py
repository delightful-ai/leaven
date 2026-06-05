"""Typed worker-stage variants accepted by the private subprocess runner."""

from ..artifacts.prompt import PromptArtifact
from ..decorators import RegisteredStage
from ..proposal import ProposalBatch
from ..stage_payloads import ProposeRequest

type WorkerStage = (
    RegisteredStage[PromptArtifact, str]
    | RegisteredStage[ProposeRequest, ProposalBatch]
)

__all__ = ["WorkerStage"]
