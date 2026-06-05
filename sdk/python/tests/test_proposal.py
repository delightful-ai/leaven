"""Tests for `leaven.proposal`."""

import pytest
from pydantic import ValidationError

from leaven._receipts import CallReceipt, QueryReceipt
from leaven.artifacts.prompt import PromptArtifact
from leaven.artifacts.skill_bank import SkillBankChangeFile, SkillBankWriteFileChange
from leaven.proposal import ProposalBatch, ProposalEffect, SkillProposal


def test_skill_proposal_lowers_to_typed_change_batch() -> None:
    """Example: parsed skill proposal output becomes a proposal batch."""

    proposal = SkillProposal(
        parent_candidate_id="cand_parent",
        surface="fp_surface_sha256_skill_bank",
        change_schema="fp_schema_sha256_skill_bank_change",
        change=SkillBankWriteFileChange(
            skill="alpha",
            path="SKILL.md",
            file=SkillBankChangeFile(content="improved"),
        ),
        read_receipts=[QueryReceipt(receipt_id="qrec_reflection")],
        effect_receipts=[CallReceipt(receipt_id="agentrec_codex")],
    )

    batch = ProposalBatch.from_skill_proposal(proposal)

    effect = batch.effects[0]
    assert effect.kind == "change"
    assert effect.parent_candidate_id == "cand_parent"
    assert effect.surface == "fp_surface_sha256_skill_bank"
    assert effect.change_schema == "fp_schema_sha256_skill_bank_change"
    assert effect.change_value == SkillBankWriteFileChange(
        skill="alpha",
        path="SKILL.md",
        file=SkillBankChangeFile(content="improved"),
    )
    assert batch.read_receipts == [QueryReceipt(receipt_id="qrec_reflection")]
    assert batch.effect_receipts == [CallReceipt(receipt_id="agentrec_codex")]


def test_proposal_effect_rejects_incomplete_change_shape() -> None:
    """Regression: proposal effects do not hide missing fields in a payload bag."""

    with pytest.raises(ValidationError, match="change proposal effects require change"):
        ProposalEffect(
            kind="change",
            parent_candidate_id="cand_parent",
            surface="fp_surface_sha256_skill_bank",
            change_schema="fp_schema_sha256_skill_bank_change",
        )


def test_proposal_effect_create_carries_direct_artifact_fields() -> None:
    """Example: create effects carry typed artifact fields directly."""

    effect = ProposalEffect.create(
        artifact_type="prompt",
        artifact_schema="fp_schema_sha256_prompt",
        artifact=PromptArtifact(template="Say hi."),
    )

    assert effect.kind == "create"
    assert effect.artifact_type == "prompt"
    assert effect.artifact_schema == "fp_schema_sha256_prompt"
    assert effect.artifact == PromptArtifact(template="Say hi.")


def test_proposal_effect_create_rejects_raw_artifact_bag() -> None:
    """Regression: create artifacts must have an explicit public owner."""

    with pytest.raises(TypeError, match="typed artifact object"):
        ProposalEffect.model_validate(
            {
                "kind": "create",
                "artifact_type": "prompt",
                "artifact_schema": "fp_schema_sha256_prompt",
                "artifact": {"template": "Say hi."},
            }
        )
