"""Tests for `scripts.check_quality_contract`."""

import importlib.util
from pathlib import Path
from types import ModuleType

ROOT = Path(__file__).resolve().parents[2]


def _load_quality_contract() -> ModuleType:
    path = ROOT / "scripts" / "check_quality_contract.py"
    spec = importlib.util.spec_from_file_location("check_quality_contract", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"could not load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


QUALITY_CONTRACT = _load_quality_contract()
defensive_type_erasure_failures_for_source = (
    QUALITY_CONTRACT.defensive_type_erasure_failures_for_source
)


def test_defensive_type_erasure_lint_rejects_failure_hiding_patterns() -> None:
    """Regression: callback output widening and fallback probes stay banned."""

    source = """
async def reward(output: object, case, cx) -> float:
    target = (case.target or {}).get("answer", "")
    return 1.0 if str(output) == target else 0.0

def decode(decoded) -> object:
    return decoded.get("result")

def stringify_value(value: object) -> str:
    return str(value)

def normalize(x: object) -> str:
    return x if isinstance(x, str) else str(x)

def ref_id(value: object) -> str:
    return getattr(value, "id", "")
"""

    failures = defensive_type_erasure_failures_for_source(_probe_path(), source)

    assert failures == [
        "tests/scripts/lint_probe.py:2: widens callback output to object",
        "tests/scripts/lint_probe.py:3: uses .get(...) on an unparsed domain value",
        "tests/scripts/lint_probe.py:4: uses str(...) to coerce a domain value",
        "tests/scripts/lint_probe.py:7: uses .get(...) on an unparsed domain value",
        "tests/scripts/lint_probe.py:10: uses str(...) to coerce a domain value",
        "tests/scripts/lint_probe.py:13: uses isinstance(..., str) else str(...) defensive fallback",
        "tests/scripts/lint_probe.py:16: uses getattr(...) to probe a domain value",
    ]


def test_defensive_type_erasure_lint_allows_declared_boundaries() -> None:
    """Example: typed mapping APIs and strict type guards are still idiomatic."""

    source = """
import os

from leaven.json_value import JsonObject

def env() -> str | None:
    return os.environ.get("LEAVEN_BIN")

def declared_mapping(row: JsonObject) -> object:
    return row.get("answer")

def strict_text(raw_output: object) -> str:
    if not isinstance(raw_output, str):
        raise TypeError("runner output must be text")
    return raw_output
"""

    assert defensive_type_erasure_failures_for_source(_probe_path(), source) == []


def _probe_path() -> Path:
    return QUALITY_CONTRACT.ROOT / "tests" / "scripts" / "lint_probe.py"
