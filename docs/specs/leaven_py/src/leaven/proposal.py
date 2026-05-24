"""Proposal types — what proposers/reflectors submit to mutate the candidate graph.

ProposalBatch carries one or more typed changes against typed artifact
surfaces. Effect kinds are `create` (fresh) and `change` (lineage-bearing).
"""

from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field

from ._receipts import CallReceipt, QueryReceipt


class ProposalEffect(BaseModel):
    """One change effect inside a ProposalBatch."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: Literal["create", "change", "change_from_agent_session"]
    parent_candidate_id: str | None = None
    """Required for 'change' and 'change_from_agent_session'."""
    surface: str
    """Locked surface fingerprint the change applies to."""
    payload: dict[str, Any]
    """Typed change payload (e.g. SkillBankChange JSON)."""
    agent_session_receipt: CallReceipt | None = None
    """Required for 'change_from_agent_session'."""


class ProposalBatch(BaseModel):
    """A batch of proposal effects submitted atomically through a stage."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    effects: list[ProposalEffect]
    read_receipts: list[QueryReceipt] = Field(default_factory=list)
    effect_receipts: list[CallReceipt] = Field(default_factory=list)

    @classmethod
    def from_skill_proposal(cls, parsed: Any) -> ProposalBatch:
        """Convenience: lower a parsed skill proposer output into a batch.

        Used by the `@lv.proposer` skill-builder convention; equivalent to
        building the batch by hand for users not on the skill path.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["ProposalBatch", "ProposalEffect"]
