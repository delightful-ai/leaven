"""Tests for `leaven.x.dspy.invoke`."""

import pytest

from leaven._receipts import CallReceipt
from leaven.x.dspy.invoke import DspyPrediction


def test_dspy_prediction_exposes_fields_and_receipt() -> None:
    prediction = DspyPrediction(
        fields={"answer": "42", "score": 1},
        leaven_lm_receipt=CallReceipt(receipt_id="lmrec_1"),
    )

    assert prediction.answer == "42"
    assert prediction.score == 1
    assert prediction.to_dict() == {"answer": "42", "score": 1}
    assert prediction.leaven_lm_receipt.receipt_id == "lmrec_1"


def test_dspy_prediction_rejects_unknown_attribute() -> None:
    prediction = DspyPrediction(
        fields={"answer": "42"},
        leaven_lm_receipt=CallReceipt(receipt_id="lmrec_1"),
    )

    with pytest.raises(AttributeError, match="missing"):
        _ = prediction.missing
