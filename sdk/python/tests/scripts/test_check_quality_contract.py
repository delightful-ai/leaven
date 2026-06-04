"""Tests for `scripts/check_quality_contract.py` defensive erasure lint."""

import importlib.util
from pathlib import Path
from typing import Protocol, cast

from _pytest.monkeypatch import MonkeyPatch

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check_quality_contract.py"


class QualityContractModule(Protocol):
    ROOT: Path
    KNOWN_DEFENSIVE_ERASURE_FAILURES: set[str]

    def check_defensive_type_erasure(self, files: list[Path] | None = None) -> list[str]: ...


def _quality_module() -> QualityContractModule:
    spec = importlib.util.spec_from_file_location("check_quality_contract", SCRIPT)
    assert spec is not None
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return cast("QualityContractModule", module)


def _check(tmp_path: Path, monkeypatch: MonkeyPatch, source: str) -> list[str]:
    module = _quality_module()
    monkeypatch.setattr(module, "ROOT", tmp_path)
    monkeypatch.setattr(module, "KNOWN_DEFENSIVE_ERASURE_FAILURES", set())
    path = tmp_path / "src" / "leaven" / "sample.py"
    path.parent.mkdir(parents=True)
    path.write_text(source, encoding="utf-8")
    return module.check_defensive_type_erasure([path])


def test_defensive_erasure_lint_detects_user_called_out_patterns(
    tmp_path: Path, monkeypatch: MonkeyPatch
) -> None:
    """Regression: the quality contract catches defensive type-erasure idioms."""

    failures = _check(
        tmp_path,
        monkeypatch,
        """
def reward(output: object, case, cx):
    assert isinstance(output, str)
    return str(output)

def target(case):
    return (case.target or {}).get("answer", "")

def payload_probe(payload):
    return payload.get("value")

def arbitrary(value):
    return getattr(value, "id", None)
""",
    )

    assert failures == [
        "src/leaven/sample.py:2: widens callback output to object",
        "src/leaven/sample.py:3: branches on isinstance(output, ...) instead of typed output",
        "src/leaven/sample.py:4: uses str(...) to coerce a domain value",
        "src/leaven/sample.py:7: uses .get(...) on an unparsed domain value",
        "src/leaven/sample.py:10: uses .get(...) on an unparsed domain value",
        "src/leaven/sample.py:13: uses getattr(...) to probe a domain value",
    ]


def test_defensive_erasure_lint_allows_declared_boundaries(
    tmp_path: Path, monkeypatch: MonkeyPatch
) -> None:
    """Example: declared mapping/env lookups and strict parse guards are not erasure."""

    failures = _check(
        tmp_path,
        monkeypatch,
        """
import os
from collections.abc import Mapping

from leaven.output import OutputContract, TextOutput

def env():
    return os.environ.get("LEAVEN_BIN")

def mapping(payload: Mapping[str, str]):
    return payload.get("value")

def parse_boundary(output):
    if not isinstance(output, str):
        raise TypeError("output must be text")
    return output

def output_contract(output: OutputContract | None):
    if isinstance(output, TextOutput):
        return output.max_chars
    return None
""",
    )

    assert failures == []


__all__ = []
