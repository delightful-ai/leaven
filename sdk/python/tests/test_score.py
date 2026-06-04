import leaven as lv


def test_score_exact_match_builds_feedback_record() -> None:
    score = lv.Score.exact_match(" Answer ", "answer")

    assert score.value == 1.0
    assert score.feedback == "exact match"


def test_score_exact_match_describes_mismatch() -> None:
    score = lv.Score.exact_match("wrong", "right")

    assert score.value == 0.0
    assert score.feedback == "expected 'right', got 'wrong'"
