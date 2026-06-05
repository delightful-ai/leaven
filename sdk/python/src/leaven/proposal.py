"""Proposal types — what proposers/reflectors submit to mutate the candidate graph.

ProposalBatch carries one or more typed changes against typed artifact
surfaces. Effect kinds are `create` (fresh) and `change` (lineage-bearing).
"""

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field, model_validator

from ._receipts import CallReceipt, QueryReceipt
from .artifacts.skill_bank import SkillBankChange, SkillBankChangeRecord
from .json_value import JsonValue

type ProposalChangeValue = JsonValue | SkillBankChangeRecord


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
    """Required for lineage-bearing change effects."""
    surface: str | None = None
    """Locked surface fingerprint the change applies to."""
    artifact_type: str | None = None
    """Required for 'create' effects."""
    artifact_schema: str | None = None
    """Required for 'create' effects."""
    artifact: JsonValue | None = None
    """Artifact value for 'create' effects."""
    change_schema: str | None = None
    """Required for all change effects."""
    change_value: ProposalChangeValue | None = None
    """Artifact-native change value for 'change' effects."""
    parser: str | None = None
    """Parser used by workspace-diff or agent-session changes."""
    agent_session_receipt: CallReceipt | None = None
    """Required for 'change_from_agent_session'."""

    @model_validator(mode="after")
    def _validate_effect_shape(self) -> "ProposalEffect":
        if self.kind == "create":
            _require(self.artifact_type, "create proposal effects require artifact_type")
            _require(self.artifact_schema, "create proposal effects require artifact_schema")
            _require(self.artifact, "create proposal effects require artifact")
            return self
        _require(self.parent_candidate_id, f"{self.kind} proposal effects require parent_candidate_id")
        _require(self.surface, f"{self.kind} proposal effects require surface")
        _require(self.change_schema, f"{self.kind} proposal effects require change_schema")
        if self.kind == "change":
            _require(self.change_value, "change proposal effects require change")
        if self.kind == "change_from_workspace_diff":
            _require(self.parser, "change_from_workspace_diff proposal effects require parser")
        if self.kind == "change_from_agent_session":
            _require(self.parser, "change_from_agent_session proposal effects require parser")
            _require(
                self.agent_session_receipt,
                "change_from_agent_session proposal effects require agent_session_receipt",
            )
        return self

    @classmethod
    def create(
        cls,
        *,
        artifact_type: str,
        artifact_schema: str,
        artifact: JsonValue,
    ) -> "ProposalEffect":
        """Build a fresh artifact creation proposal effect."""
        return cls(
            kind="create",
            artifact_type=artifact_type,
            artifact_schema=artifact_schema,
            artifact=artifact,
        )

    @classmethod
    def change(
        cls,
        *,
        parent_candidate_id: str,
        surface: str,
        change_schema: str,
        change: ProposalChangeValue,
    ) -> "ProposalEffect":
        """Build a lineage-bearing change proposal effect."""
        return cls(
            kind="change",
            parent_candidate_id=parent_candidate_id,
            surface=surface,
            change_schema=change_schema,
            change_value=change,
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
            change_schema=change_schema,
            parser=parser,
            agent_session_receipt=agent_session_receipt,
        )


class ProposalBatch(BaseModel):
    """A batch of proposal effects submitted atomically through a stage."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    effects: list[ProposalEffect]
    read_receipts: list[QueryReceipt] = Field(default_factory=list)
    effect_receipts: list[CallReceipt] = Field(default_factory=list)

    @classmethod
    def from_skill_proposal(cls, proposal: "SkillProposal") -> "ProposalBatch":
        """Convenience: lower a parsed skill proposer output into a batch.

        Used by the `@lv.proposer` skill-builder convention; equivalent to
        building the batch by hand for users not on the skill path.
        """
        return cls(
            effects=[
                ProposalEffect.change(
                    parent_candidate_id=proposal.parent_candidate_id,
                    surface=proposal.surface,
                    change_schema=proposal.change_schema,
                    change=proposal.change,
                )
            ],
            read_receipts=list(proposal.read_receipts),
            effect_receipts=list(proposal.effect_receipts),
        )


class SkillProposal(BaseModel):
    """Parsed structured output for a skill-bank proposal stage."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    parent_candidate_id: str
    surface: str
    change_schema: str
    change: SkillBankChange
    read_receipts: list[QueryReceipt] = Field(default_factory=list)
    effect_receipts: list[CallReceipt] = Field(default_factory=list)


def _require[T](value: T | None, message: str) -> T:
    if value is None:
        raise ValueError(message)
    return value


__all__ = ["ProposalBatch", "ProposalChangeValue", "ProposalEffect", "SkillProposal"]
