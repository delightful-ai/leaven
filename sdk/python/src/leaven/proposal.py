"""Proposal types — what proposers/reflectors submit to mutate the candidate graph.

ProposalBatch carries one or more typed changes against typed artifact
surfaces. Effect kinds are `create` (fresh) and `change` (lineage-bearing).
"""

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

from ._receipts import CallReceipt, QueryReceipt
from .json_value import JsonObject


class ProposalEffect(BaseModel):
    """One change effect inside a ProposalBatch."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: Literal[
        "create",
        "change",
        "change_from_workspace_diff",
        "change_from_agent_session",
    ]
    parent_candidate_id: str | None = None
    """Required for 'change' and 'change_from_agent_session'."""
    surface: str
    """Locked surface fingerprint the change applies to."""
    payload: JsonObject
    """Typed change payload (e.g. SkillBankChange JSON)."""
    agent_session_receipt: CallReceipt | None = None
    """Required for 'change_from_agent_session'."""

    @classmethod
    def change(
        cls,
        *,
        parent_candidate_id: str,
        surface: str,
        change_schema: str,
        change: JsonObject,
    ) -> "ProposalEffect":
        """Build a lineage-bearing change proposal effect."""
        return cls(
            kind="change",
            parent_candidate_id=parent_candidate_id,
            surface=surface,
            payload={"change_schema": change_schema, "change": change},
        )

    @classmethod
    def change_from_agent_session(
        cls,
        *,
        parent_candidate_id: str,
        surface: str,
        change_schema: str,
        parser: str,
        agent_session_receipt: CallReceipt,
    ) -> "ProposalEffect":
        """Build a proposal effect bound to a prior `cx.agent.run` receipt."""
        return cls(
            kind="change_from_agent_session",
            parent_candidate_id=parent_candidate_id,
            surface=surface,
            payload={"change_schema": change_schema, "parser": parser},
            agent_session_receipt=agent_session_receipt,
        )


class ProposalBatch(BaseModel):
    """A batch of proposal effects submitted atomically through a stage."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    effects: list[ProposalEffect]
    read_receipts: list[QueryReceipt] = Field(default_factory=list)
    effect_receipts: list[CallReceipt] = Field(default_factory=list)

    @classmethod
    def from_skill_proposal(cls, parsed: object) -> "ProposalBatch":
        """Convenience: lower a parsed skill proposer output into a batch.

        Used by the `@lv.proposer` skill-builder convention; equivalent to
        building the batch by hand for users not on the skill path.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["ProposalBatch", "ProposalEffect"]
