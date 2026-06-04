import pytest

from leaven._receipts import CallReceipt, QueryReceipt
from leaven._seam._wire.results import ProposalBatchPrimary, ProposalSubmitResult
from leaven.builders.proposals import ProposalsBuilder
from leaven.proposal import ProposalBatch, ProposalEffect


@pytest.mark.asyncio
async def test_proposals_builder_submits_agent_session_batch_through_seam() -> None:
    """Scenario: proposer submits a typed batch citing a prior agent run."""

    client = FakeProposalSeamClient()
    proposals = ProposalsBuilder._for_seam(
        client,
        idempotency_prefix="proposal-builder-test",
        plan_id="planproposalbuilder001",
    )
    batch = ProposalBatch(
        effects=[
            ProposalEffect.change_from_agent_session(
                parent_candidate_id="cand_parent",
                surface="fp_surface_sha256_program",
                change_schema="fp_schema_sha256_skill_patch",
                parser="leaven.agent_session.skill_patch.v1",
                agent_session_receipt=CallReceipt(receipt_id="agentrec_codex"),
            )
        ],
        read_receipts=[QueryReceipt(receipt_id="qrec_reflection")],
    )

    submission = await proposals.submit(batch)

    assert client.request_value["method"] == "leaven/proposal.submit_batch"
    params = client.request_value["params"]
    assert params["plan_id"] == "planproposalbuilder001"
    assert params["return"] == ["proposal_batch"]
    op = params["ops"][0]
    assert op["kind"] == "write"
    assert op["idempotency_key"] == "proposal-builder-test-submit"
    assert op["write"]["kind"] == "submit_proposal_batch"
    proposal = op["write"]["proposals"][0]
    assert proposal["effect"] == {
        "kind": "change_from_agent_session",
        "target": "cand_parent",
        "surface_fingerprint": "fp_surface_sha256_program",
        "change_schema": "fp_schema_sha256_skill_patch",
        "parser": "leaven.agent_session.skill_patch.v1",
        "agent_receipt": "agentrec_codex",
    }
    assert proposal["causal"] == {"inputs": ["cand_parent"]}
    assert proposal["read_receipts"] == ["qrec_reflection", "agentrec_codex"]
    assert proposal["informed_by"] == {
        "kind": "literal",
        "value": ["qrec_reflection", "agentrec_codex"],
    }
    assert submission.receipt.receipt_id == "wrec_proposal_submit"
    assert submission.proposal_ids == ["prop_submitted"]


@pytest.mark.asyncio
async def test_proposals_builder_requires_bound_seam_client() -> None:
    """Regression: unbound public builders remain explicit scaffold."""

    with pytest.raises(NotImplementedError, match="engine-bound public-seam client"):
        await ProposalsBuilder().submit(
            ProposalBatch(
                effects=[
                    ProposalEffect.change(
                        parent_candidate_id="cand_parent",
                        surface="fp_surface_sha256_program",
                        change_schema="fp_schema_sha256_patch",
                        change={"patch": "demo"},
                    )
                ],
            )
        )


class FakeProposalSeamClient:
    def __init__(self) -> None:
        self.request_value: dict = {}

    def proposal_submit(self, request: dict) -> ProposalSubmitResult:
        self.request_value = request
        return ProposalSubmitResult(
            method="leaven/proposal.submit_batch",
            primary=ProposalBatchPrimary(
                kind="proposal_batch_receipt",
                batch_id="pb_submitted",
                proposal_ids=["prop_submitted"],
                status="committed",
                receipt="wrec_proposal_submit",
            ),
            receipts=[],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["public"],
        )
