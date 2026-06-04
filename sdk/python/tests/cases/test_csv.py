"""Tests for `leaven.cases.csv` public case loading."""

from pathlib import Path

import pytest

from leaven.cases import from_csv


def test_from_csv_loads_selected_columns_into_case_fields(tmp_path: Path) -> None:
    """Example: CSV rows become typed Case records with explicit field owners."""

    path = tmp_path / "train.csv"
    path.write_text(
        "id,question,answer,split\ncsv-1,2 + 5?,7,train\n",
        encoding="utf-8",
    )

    case_set = from_csv(
        str(path),
        input_columns=["question"],
        target_columns=["answer"],
        metadata_columns=["split"],
    )

    assert case_set.name == "train"
    assert len(case_set.cases) == 1
    case = case_set.cases[0]
    assert case.id == "csv-1"
    assert case.input == {"question": "2 + 5?"}
    assert case.target == {"answer": "7"}
    assert case.metadata == {"split": "train"}


def test_from_csv_rejects_missing_input_columns(tmp_path: Path) -> None:
    """Regression: CSV loading refuses an unowned empty input shape."""

    path = tmp_path / "bad.csv"
    path.write_text("id,answer\ncsv-bad,7\n", encoding="utf-8")

    with pytest.raises(ValueError, match="input_columns must name at least one column"):
        from_csv(str(path), target_columns=["answer"])


__all__ = []
