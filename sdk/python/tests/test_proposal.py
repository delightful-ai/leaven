"""Tests for `leaven.proposal`."""

from leaven._receipts import CallReceipt, QueryReceipt
from leaven.proposal import ProposalBatch, SkillProposal


def test_skill_proposal_lowers_to_typed_change_batch() -> None:
    """Example: parsed skill proposal output becomes a proposal batch."""

    proposal = SkillProposal(
        parent_candidate_id="cand_parent",
        surface="fp_surface_sha256_skill_bank",
        change_schema="fp_schema_sha256_skill_bank_change",
        change={"files": {"alpha/SKILL.md": "improved"}},
        read_receipts=[QueryReceipt(receipt_id="qrec_reflection")],
        effect_receipts=[CallReceipt(receipt_id="agentrec_codex")],
    )

    batch = ProposalBatch.from_skill_proposal(proposal)

    effect = batch.effects[0]
    assert effect.kind == "change"
    assert effect.parent_candidate_id == "cand_parent"
    assert effect.surface == "fp_surface_sha256_skill_bank"
    assert effect.payload == {
        "change_schema": "fp_schema_sha256_skill_bank_change",
        "change": {"files": {"alpha/SKILL.md": "improved"}},
    }
    assert batch.read_receipts == [QueryReceipt(receipt_id="qrec_reflection")]
    assert batch.effect_receipts == [CallReceipt(receipt_id="agentrec_codex")]
