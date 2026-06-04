"""Tests for `leaven.cases.jsonl` public case loading."""

import json
from pathlib import Path

import pytest

from leaven.cases import from_jsonl


def test_from_jsonl_loads_json_object_cases(tmp_path: Path) -> None:
    """Example: JSONL rows become typed Case records with JSON object fields."""

    path = tmp_path / "train.jsonl"
    path.write_text(
        json.dumps(
            {
                "id": "example-1",
                "input": {"question": "2 + 2?"},
                "target": {"answer": "4"},
                "metadata": {"split": "train"},
            }
        )
        + "\n",
        encoding="utf-8",
    )

    case_set = from_jsonl(str(path))

    assert case_set.name == "train"
    assert len(case_set.cases) == 1
    case = case_set.cases[0]
    assert case.id == "example-1"
    assert case.input == {"question": "2 + 2?"}
    assert case.target == {"answer": "4"}
    assert case.metadata == {"split": "train"}


def test_from_jsonl_rejects_non_object_input(tmp_path: Path) -> None:
    """Regression: loader does not erase arbitrary JSON into Case.input."""

    path = tmp_path / "bad.jsonl"
    path.write_text(
        json.dumps({"id": "bad", "input": ["not", "an", "object"]}) + "\n",
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="input must be a JSON object"):
        from_jsonl(str(path))


__all__ = []
