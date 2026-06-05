"""Tests for typed durable-seam optimize receipt projections."""

import msgspec
import pytest

from leaven._seam._wire.payloads import StageRunResult
from leaven._seam_optimize.receipts import (
    effect_cost_totals_from_stage_result,
    effect_receipts_from_stage_result,
    proposal_receipts_from_stage_result,
)


def test_receipt_projection_consumes_typed_stage_run_result() -> None:
    """Scenario: typed stage output projects public SDK receipt handles."""

    result = msgspec.json.decode(
        (
            b'{"schema_version":"leaven.stage_run.v1","message":"stage_run_result",'
            b'"stage":"proposer","stage_call_id":"sc_1",'
            b'"output":{"kind":"text","summary":"ok","visibility":"public",'
            b'"data_classes":["public"]},'
            b'"effect_receipts":[{"method":"leaven/agent.run","receipt":"agentrec_1",'
            b'"cost":{"usd_micro":2500,"input_tokens":7,"output_tokens":11},'
            b'"blob_refs":[{"kind":"blob_ref","id":"blob_transcript","sha256":"abc",'
            b'"bytes":12,"data_classes":["public"]}]}],'
            b'"proposal_receipts":[{"method":"leaven/proposal.submit_batch",'
            b'"receipt":{"kind":"receipt","id":"wrec_1"},'
            b'"proposal_ids":["prop_1","prop_2"]}]}'
        ),
        type=StageRunResult,
    )

    effects = effect_receipts_from_stage_result(result)
    proposals = proposal_receipts_from_stage_result(result)
    costs = effect_cost_totals_from_stage_result(result)

    assert effects[0].receipt_id == "agentrec_1"
    assert effects[0].blob_refs[0].blob_id == "blob_transcript"
    assert proposals[0].receipt_id == "wrec_1"
    assert proposals[0].proposal_ids == ["prop_1", "prop_2"]
    assert costs.cost_usd == 0.0025
    assert costs.lm_tokens == 18


def test_receipt_projection_rejects_negative_costs() -> None:
    """Regression: typed cost fields must not be silently coerced to zero."""

    result = msgspec.json.decode(
        (
            b'{"schema_version":"leaven.stage_run.v1","message":"stage_run_result",'
            b'"stage":"runner","stage_call_id":"sc_1",'
            b'"output":{"kind":"text","summary":"ok","visibility":"public",'
            b'"data_classes":["public"]},'
            b'"effect_receipts":[{"method":"leaven/lm.complete","receipt":"lmrec_1",'
            b'"cost":{"usd_micro":-1}}]}'
        ),
        type=StageRunResult,
    )

    with pytest.raises(ValueError, match="cost values must be nonnegative"):
        effect_cost_totals_from_stage_result(result)


__all__ = []
