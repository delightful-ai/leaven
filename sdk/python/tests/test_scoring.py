import math

import leaven as lv


def test_exact_match_uses_default_text_normalization() -> None:
    assert lv.scoring.exact_match(" Answer ", "answer") == 1.0
    assert lv.scoring.exact_match("answer", "different") == 0.0


def test_normalized_match_can_disable_normalizers() -> None:
    assert lv.scoring.normalized_match(" Answer ", "answer", strip=False) == 0.0
    assert lv.scoring.normalized_match("Answer", "answer", lowercase=False) == 0.0


def test_multi_tolerance_scores_relative_numeric_error() -> None:
    assert lv.scoring.multi_tolerance("100", "100") == 1.0
    assert math.isclose(lv.scoring.multi_tolerance("105", "100"), 0.4)
    assert lv.scoring.multi_tolerance("not numeric", "100") == 0.0
    assert lv.scoring.multi_tolerance("1", "0") == 0.0


def test_f1_scores_token_overlap() -> None:
    assert lv.scoring.f1("red blue", "blue red") == 1.0
    assert math.isclose(lv.scoring.f1("red blue", "red green"), 0.5)
    assert lv.scoring.f1("", "red") == 0.0

