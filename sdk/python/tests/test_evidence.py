import pytest

import leaven as lv
from leaven.evidence import EvidenceEnvelope, EvidencePrivate, EvidencePublic, EvidencePublicPayload


def test_public_only_builds_public_projection() -> None:
    envelope = EvidenceEnvelope.public_only(
        payload=EvidencePublicPayload(feedback="clear"),
        data_classes=[lv.data_class.OPTIMIZER_VISIBLE],
    )

    assert envelope.public is not None
    assert envelope.public.payload == EvidencePublicPayload(feedback="clear")
    assert envelope.public.data_classes == [lv.data_class.OPTIMIZER_VISIBLE]
    assert envelope.private is None
    assert envelope.target_derived is False


def test_public_private_splits_data_classes_from_payloads() -> None:
    envelope = EvidenceEnvelope.public_private(
        public=EvidencePublic(
            data_classes=[lv.data_class.OPTIMIZER_VISIBLE],
            payload=EvidencePublicPayload(feedback="clear"),
        ),
        private=EvidencePrivate(
            visibility="evaluator_only",
            data_classes=[lv.data_class.CASE_TARGET],
            payload={"rubric": "exact"},
        ),
        target_derived=True,
    )

    assert envelope.public is not None
    assert envelope.public.payload == EvidencePublicPayload(feedback="clear")
    assert envelope.private is not None
    assert envelope.private.payload == {"rubric": "exact"}
    assert envelope.private.visibility == "evaluator_only"
    assert envelope.target_derived is True


def test_target_private_evidence_must_be_declared() -> None:
    with pytest.raises(ValueError, match="target_derived=True"):
        EvidenceEnvelope.public_private(
            public=EvidencePublic(data_classes=[lv.data_class.OPTIMIZER_VISIBLE]),
            private=EvidencePrivate(data_classes=[lv.data_class.CASE_TARGET]),
        )


def test_public_payload_rejects_unknown_fields_at_construction() -> None:
    with pytest.raises(ValueError, match="extra_forbidden"):
        EvidencePublicPayload.model_validate({"output": "42"})
