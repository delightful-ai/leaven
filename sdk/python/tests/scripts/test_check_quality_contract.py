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
check_no_public_builder_notimplemented = QUALITY_CONTRACT.check_no_public_builder_notimplemented
check_no_production_notimplemented = QUALITY_CONTRACT.check_no_production_notimplemented


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
        "tests/scripts/lint_probe.py:2: LEAVEN001 widens domain value `output` to object",
        "tests/scripts/lint_probe.py:3: LEAVEN005 uses .get(...) on an unparsed domain value",
        "tests/scripts/lint_probe.py:4: LEAVEN002 uses str(...) to coerce a domain value",
        "tests/scripts/lint_probe.py:7: LEAVEN005 uses .get(...) on an unparsed domain value",
        "tests/scripts/lint_probe.py:9: LEAVEN001 widens domain value `value` to object",
        "tests/scripts/lint_probe.py:10: LEAVEN002 uses str(...) to coerce a domain value",
        "tests/scripts/lint_probe.py:13: LEAVEN003 uses isinstance(..., str) else str(...) defensive fallback",
        "tests/scripts/lint_probe.py:15: LEAVEN001 widens domain value `value` to object",
        "tests/scripts/lint_probe.py:16: LEAVEN006 uses getattr(...) to probe a domain value",
        "tests/scripts/lint_probe.py:19: LEAVEN005 uses .get(...) on an unparsed domain value",
    ]


def test_defensive_type_erasure_lint_allows_environment_reads_and_strict_guards() -> None:
    """Example: environment reads and strict type guards are still idiomatic."""

    source = """
import os

def env() -> str | None:
    return os.environ.get("LEAVEN_BIN")

def strict_text(raw_output: object) -> str:  # noqa: LEAVEN001 -- explicit ingress guard parses unknown worker output
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


def test_domain_object_widening_noqa_requires_code_and_justification() -> None:
    """Regression: `object` domain slots need an auditable local exception."""

    source = """
def bare(payload: object) -> None:
    pass

def code_only(result: object) -> None:  # noqa: LEAVEN001
    pass

def justified(value: object) -> None:  # noqa: LEAVEN001 -- third-party SDK callback is untyped until adapter lands
    pass
"""

    failures = defensive_type_erasure_failures_for_source(_probe_path(), source)

    assert failures == [
        "tests/scripts/lint_probe.py:2: LEAVEN001 widens domain value `payload` to object",
        "tests/scripts/lint_probe.py:5: LEAVEN001 widens domain value `result` to object",
    ]


def test_public_builder_notimplemented_lint_rejects_scaffold_bodies(
    tmp_path: Path,
) -> None:
    """Regression: public builder modules must not carry scaffold errors."""

    builder_root = tmp_path / "builders"
    builder_root.mkdir()
    (builder_root / "agent.py").write_text(
        """
class AgentBuilder:
    async def run(self) -> None:
        raise NotImplementedError("scaffold")
""",
        encoding="utf-8",
    )
    (builder_root / "_private.py").write_text(
        "raise NotImplementedError('private sentinel')\n",
        encoding="utf-8",
    )
    failures = check_no_public_builder_notimplemented(builder_root)

    assert failures == [
        f"{builder_root / 'agent.py'} contains NotImplementedError in a public builder module"
    ]


def test_production_notimplemented_lint_rejects_source_examples_and_codegen(
    tmp_path: Path,
) -> None:
    """Regression: production SDK code must use typed errors, not scaffolds."""

    sdk_root = tmp_path
    for part in ("src/leaven", "examples", "codegen", "tests"):
        (sdk_root / part).mkdir(parents=True)
    (sdk_root / "src/leaven/optimize.py").write_text(
        "raise NotImplementedError('public scaffold')\n",
        encoding="utf-8",
    )
    (sdk_root / "examples/01_demo.py").write_text(
        "raise NotImplementedError('example scaffold')\n",
        encoding="utf-8",
    )
    (sdk_root / "codegen/gen.py").write_text(
        "raise NotImplementedError('codegen scaffold')\n",
        encoding="utf-8",
    )
    (sdk_root / "tests/test_fixture.py").write_text(
        "raise NotImplementedError('test fixture is allowed')\n",
        encoding="utf-8",
    )

    failures = check_no_production_notimplemented(
        (
            sdk_root / "src" / "leaven",
            sdk_root / "examples",
            sdk_root / "codegen",
        )
    )

    assert failures == [
        f"{sdk_root / 'src/leaven/optimize.py'} contains NotImplementedError in production SDK code",
        f"{sdk_root / 'examples/01_demo.py'} contains NotImplementedError in production SDK code",
        f"{sdk_root / 'codegen/gen.py'} contains NotImplementedError in production SDK code",
    ]


def _probe_path() -> Path:
    return QUALITY_CONTRACT.ROOT / "tests" / "scripts" / "lint_probe.py"
