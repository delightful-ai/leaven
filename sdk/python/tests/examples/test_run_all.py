"""Tests for the examples tour runner."""

import importlib.util
from pathlib import Path
from typing import Protocol, cast


class _RunAllModule(Protocol):
    def _is_expected_boundary_error(self, script_name: str, error: Exception) -> bool: ...


def _run_all_module() -> _RunAllModule:
    path = Path(__file__).parents[2] / "examples" / "run_all.py"
    spec = importlib.util.spec_from_file_location("leaven_examples_run_all", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load examples/run_all.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return cast("_RunAllModule", module)


def test_examples_runner_rejects_product_boundary_errors() -> None:
    runner = _run_all_module()

    assert not runner._is_expected_boundary_error(
        "03_prompt_optimize.py",
        NotImplementedError("product example must not scaffold"),
    )
    assert not runner._is_expected_boundary_error(
        "10_live_codex_seam.py",
        NotImplementedError("live substrate proof must not scaffold"),
    )


def test_examples_runner_allows_only_named_scaffold_boundaries() -> None:
    runner = _run_all_module()

    assert runner._is_expected_boundary_error(
        "04_evoskill_skill_bank.py",
        TypeError("this slice optimizes a PromptArtifact seed only"),
    )
    assert not runner._is_expected_boundary_error(
        "04_evoskill_skill_bank.py",
        NotImplementedError("wrong boundary kind"),
    )
    assert not runner._is_expected_boundary_error(
        "08_dspy_dropin.py",
        NotImplementedError("removed optional adapter scaffold"),
    )
