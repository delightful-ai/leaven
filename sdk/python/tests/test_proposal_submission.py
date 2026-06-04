import pytest

from leaven._receipts import CallReceipt, QueryReceipt
from leaven._seam import ProposalSubmitRequest
from leaven._seam._wire.results import ProposalBatchPrimary, ProposalSubmitResult
from leaven.builders.proposals import ProposalsBuilder
from leaven.json_value import JsonObject, JsonValue
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

    assert client.request_value.method == "leaven/proposal.submit_batch"
    params = client.request_value.to_params()
    assert params["plan_id"] == "planproposalbuilder001"
    assert params["return"] == ["proposal_batch"]
    ops = _json_array(params["ops"])
    op = _json_object(ops[0])
    assert op["kind"] == "write"
    assert op["idempotency_key"] == "proposal-builder-test-submit"
    write = _json_object(op["write"])
    assert write["kind"] == "submit_proposal_batch"
    proposals = _json_array(write["proposals"])
    proposal = _json_object(proposals[0])
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


def _json_object(value: JsonValue) -> JsonObject:
    assert isinstance(value, dict)
    return value


def _json_array(value: JsonValue) -> list[JsonValue]:
    assert isinstance(value, list)
    return value


class FakeProposalSeamClient:
    def __init__(self) -> None:
        self.request_value = ProposalSubmitRequest(
            request_id="unset",
            plan_id="unset",
            idempotency_key="unset",
            proposals=[],
        )

    def proposal_submit(self, request: ProposalSubmitRequest) -> ProposalSubmitResult:
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
