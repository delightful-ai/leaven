"""Tests for generated public-seam graph-write records."""

import msgspec
import pytest

from leaven._seam._wire.expressions import ValueExprLiteral, ValueExprVar
from leaven._seam._wire.payloads import (
    PlanDocument,
)
from leaven._seam._wire.writes import (
    ProposalEffectChange,
    ProposalEffectCreate,
    SubmitProposalBatchWrite,
)


def test_submit_proposal_batch_write_decodes_typed_effect_value_exprs() -> None:
    """Example: proposal effects and annotations keep their ValueExpr variants."""

    decoded = msgspec.json.decode(_proposal_plan(), type=PlanDocument)
    write = decoded.ops[0].write

    assert isinstance(write, SubmitProposalBatchWrite)
    create = write.proposals[0]
    change = write.proposals[1]
    assert isinstance(create.effect, ProposalEffectCreate)
    assert isinstance(create.effect.artifact, ValueExprLiteral)
    assert create.effect.artifact.value == "new prompt"
    assert isinstance(create.annotations, ValueExprVar)
    assert create.annotations.name == "annotation_value"
    assert isinstance(change.effect, ProposalEffectChange)
    assert isinstance(change.effect.change, ValueExprLiteral)
    assert change.effect.change.value == "patch text"


def test_submit_proposal_batch_rejects_unknown_effect_kind() -> None:
    """Boundary check: proposal effects are tagged records, not raw objects."""

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(
            _proposal_plan().replace(b'"kind":"create"', b'"kind":"mystery"', 1),
            type=PlanDocument,
        )


def _proposal_plan() -> bytes:
    return (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"write","name":"proposal_batch",'
        b'"idempotency_key":"idem_1",'
        b'"write":{"kind":"submit_proposal_batch","semantics":"sequence","proposals":['
        b'{"effect":{"kind":"create","artifact_type":"prompt",'
        b'"artifact_schema":"fp_schema_artifact",'
        b'"artifact":{"kind":"literal","value":"new prompt"}},'
        b'"causal":{},"informed_by":{"kind":"var","name":"seed"},'
        b'"annotations":{"kind":"var","name":"annotation_value"}},'
        b'{"effect":{"kind":"change","target":"cand_seed",'
        b'"surface_fingerprint":"fp_surface","change_schema":"fp_schema_change",'
        b'"change":{"kind":"literal","value":"patch text"}},'
        b'"causal":{},"informed_by":{"kind":"var","name":"seed"}}'
        b']} }],'
        b'"return":["proposal_batch"],"commit":{"kind":"graph_writes_atomic","on_stale":"reject"}}'
    )


__all__ = []
