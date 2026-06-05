"""Tests for generated public-seam method metadata."""

import subprocess
import sys
from pathlib import Path

from leaven._seam._wire.methods import METHOD_BINDINGS


def test_generated_methods_match_current_rust_source() -> None:
    subprocess.run(
        [sys.executable, "codegen/generate_seam_wire.py", "--check"],
        check=True,
        cwd=Path(__file__).resolve().parents[3],
    )


def test_generated_bindings_include_schema_hashes_and_actions() -> None:
    bindings = {binding.method: binding for binding in METHOD_BINDINGS}

    assert bindings["leaven/proposal.apply"].required_action == "proposal.apply_batch"
    assert bindings["leaven/stage.run"].params_schema == "leaven.stage_run.v1.schema.json"
    assert bindings["leaven/stage.run"].params_schema_fingerprint.startswith("fp_schema_sha256_")
    assert bindings["leaven/event.emit"].produces_receipt is True
