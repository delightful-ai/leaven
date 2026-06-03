"""`cx.assessments.*` — submit assessment batches at the end of evaluation."""

from collections.abc import Sequence

from pydantic import BaseModel, ConfigDict

from .._receipts import WriteReceipt
from ..assessment import AssessmentWrite


class AssessmentSubmission(BaseModel):
    """Receipt for a submit_assessments call. Per-assessment receipts are inside."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    receipt: WriteReceipt
    submitted: int
    """Count of assessments admitted."""


class AssessmentsBuilder:
    """Assessment submission bound to a context."""

    async def submit(
        self,
        evaluation_request_id: str,
        assessments: Sequence[AssessmentWrite],
    ) -> AssessmentSubmission:
        """Submit a batch of assessments against an evaluation request.

        The batch is admitted atomically: either every assessment passes
        seam validation and admits, or the whole batch is rejected with
        per-assessment denial details.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["AssessmentSubmission", "AssessmentsBuilder"]
