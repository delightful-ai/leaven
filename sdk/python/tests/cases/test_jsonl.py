"""Tests for `leaven.cases.jsonl` public case loading."""

import json
from pathlib import Path

import pytest

from leaven.cases import from_iterable, from_jsonl


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


def test_from_iterable_builds_inline_case_set() -> None:
    """Example: inline JSON object rows use the same Case projection as JSONL."""

    case_set = from_iterable(
        [
            {
                "id": "inline-1",
                "input": {"question": "3 + 4?"},
                "target": {"answer": "7"},
                "metadata": {"split": "train"},
            }
        ],
        name="inline-train",
    )

    assert case_set.name == "inline-train"
    assert len(case_set.cases) == 1
    case = case_set.cases[0]
    assert case.id == "inline-1"
    assert case.input == {"question": "3 + 4?"}
    assert case.target == {"answer": "7"}
    assert case.metadata == {"split": "train"}


def test_from_iterable_rejects_non_object_target() -> None:
    """Regression: inline loader does not erase arbitrary target JSON."""

    with pytest.raises(ValueError, match="optional JSON object field must be a JSON object"):
        from_iterable([{"id": "bad-target", "input": {"question": "?"}, "target": "nope"}])


__all__ = []
