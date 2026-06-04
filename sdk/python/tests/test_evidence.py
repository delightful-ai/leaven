import pytest

import leaven as lv
from leaven.evidence import EvidenceEnvelope


def test_public_only_builds_public_projection() -> None:
    envelope = EvidenceEnvelope.public_only(
        payload={"feedback": "clear"},
        data_classes=[lv.data_class.OPTIMIZER_VISIBLE],
    )

    assert envelope.public is not None
    assert envelope.public.payload == {"feedback": "clear"}
    assert envelope.public.data_classes == [lv.data_class.OPTIMIZER_VISIBLE]
    assert envelope.private is None
    assert envelope.target_derived is False


def test_public_private_splits_data_classes_from_payloads() -> None:
    envelope = EvidenceEnvelope.public_private(
        public={
            "feedback": "clear",
            "data_classes": [lv.data_class.OPTIMIZER_VISIBLE],
        },
        private={
            "rubric": "exact",
            "visibility": "evaluator_only",
            "data_classes": [lv.data_class.CASE_TARGET],
        },
        target_derived=True,
    )

    assert envelope.public is not None
    assert envelope.public.payload == {"feedback": "clear"}
    assert envelope.private is not None
    assert envelope.private.payload == {"rubric": "exact"}
    assert envelope.private.visibility == "evaluator_only"
    assert envelope.target_derived is True


def test_target_private_evidence_must_be_declared() -> None:
    with pytest.raises(ValueError, match="target_derived=True"):
        EvidenceEnvelope.public_private(
            public={"data_classes": [lv.data_class.OPTIMIZER_VISIBLE]},
            private={"data_classes": [lv.data_class.CASE_TARGET]},
        )

