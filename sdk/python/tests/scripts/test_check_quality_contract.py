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

def declared_mapping(row: JsonObject) -> object:
    return row.get("answer")
"""

    failures = defensive_type_erasure_failures_for_source(_probe_path(), source)

    assert failures == [
        "tests/scripts/lint_probe.py:2: LEAVEN001 widens callback output to object",
        "tests/scripts/lint_probe.py:3: LEAVEN005 uses .get(...) on an unparsed domain value",
        "tests/scripts/lint_probe.py:4: LEAVEN002 uses str(...) to coerce a domain value",
        "tests/scripts/lint_probe.py:7: LEAVEN005 uses .get(...) on an unparsed domain value",
        "tests/scripts/lint_probe.py:10: LEAVEN002 uses str(...) to coerce a domain value",
        "tests/scripts/lint_probe.py:13: LEAVEN003 uses isinstance(..., str) else str(...) defensive fallback",
        "tests/scripts/lint_probe.py:16: LEAVEN006 uses getattr(...) to probe a domain value",
        "tests/scripts/lint_probe.py:19: LEAVEN005 uses .get(...) on an unparsed domain value",
    ]


def test_defensive_type_erasure_lint_allows_environment_reads_and_strict_guards() -> None:
    """Example: environment reads and strict type guards are still idiomatic."""

    source = """
import os

def env() -> str | None:
    return os.environ.get("LEAVEN_BIN")

def strict_text(raw_output: object) -> str:
    if not isinstance(raw_output, str):
        raise TypeError("runner output must be text")
    return raw_output
"""

    assert defensive_type_erasure_failures_for_source(_probe_path(), source) == []


def test_defensive_type_erasure_noqa_requires_code_and_justification() -> None:
    """Regression: custom Leaven suppressions stay explicit and auditable."""

    source = """
def bare(decoded) -> object:
    return decoded.get("result")  # noqa

def code_only(decoded) -> object:
    return decoded.get("result")  # noqa: LEAVEN005

def justified(decoded) -> object:
    return decoded.get("result")  # noqa: LEAVEN005 -- third-party schema probe pending typed adapter
"""

    failures = defensive_type_erasure_failures_for_source(_probe_path(), source)

    assert failures == [
        "tests/scripts/lint_probe.py:3: LEAVEN005 uses .get(...) on an unparsed domain value",
        "tests/scripts/lint_probe.py:6: LEAVEN005 uses .get(...) on an unparsed domain value",
    ]


def _probe_path() -> Path:
    return QUALITY_CONTRACT.ROOT / "tests" / "scripts" / "lint_probe.py"
