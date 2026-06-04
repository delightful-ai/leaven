"""Tests for `leaven._runs.rust_export`."""

from pathlib import Path

import pytest

from leaven._runs.rust_export import load_rust_run_readback


def test_load_rust_run_readback_skips_runs_without_latest_checkpoint(tmp_path: Path) -> None:
    run_dir = tmp_path / "run"
    run_dir.mkdir()

    assert load_rust_run_readback(run_dir, leaven_bin=tmp_path / "missing") is None


def test_load_rust_run_readback_invokes_leaven_run_inspect(tmp_path: Path) -> None:
    run_dir = tmp_path / "run"
    (run_dir / "checkpoints").mkdir(parents=True)
    (run_dir / "checkpoints" / "LATEST").write_text("checkpoint_1\n", encoding="utf-8")
    calls = tmp_path / "calls.txt"
    fake = tmp_path / "leaven"
    fake.write_text(
        "#!/bin/sh\n"
        "printf '%s\\n' \"$@\" > \"$LEAVEN_TEST_CALLS\"\n"
        "cat <<'JSON'\n"
        "{\n"
        '  "schema_version": "leaven.run_inspection_export.v1",\n'
        '  "run_id": "run_1",\n'
        '  "latest_checkpoint": "checkpoint_1",\n'
        '  "checkpoint": {\n'
        '    "format_version": 1,\n'
        '    "graph_snapshot": {\n'
        '      "store": "file",\n'
        '      "key": "graph.blob",\n'
        '      "schema": "060606",\n'
        '      "format": "Json"\n'
        "    },\n"
        '    "artifact_ref_count": 0,\n'
        '    "evidence_ref_count": 0,\n'
        '    "stage_journal_ref_count": 0,\n'
        '    "workspace_journal_ref_count": 0,\n'
        '    "has_optimizer_state": false,\n'
        '    "has_cache_index": false\n'
        "  },\n"
        '  "graph": {\n'
        '    "blob": {\n'
        '      "store": "file",\n'
        '      "key": "graph.blob",\n'
        '      "schema": "060606",\n'
        '      "format": "Json"\n'
        "    },\n"
        '    "bytes": 128,\n'
        '    "run_id": "run_graph",\n'
        '    "candidate_count": 2,\n'
        '    "proposal_batch_count": 1,\n'
        '    "proposal_count": 1,\n'
        '    "apply_attempt_count": 1,\n'
        '    "evaluation_request_count": 1,\n'
        '    "assessment_count": 2,\n'
        '    "event_count": 3\n'
        "  }\n"
        "}\n"
        "JSON\n",
        encoding="utf-8",
    )
    fake.chmod(0o755)

    with pytest.MonkeyPatch.context() as monkeypatch:
        monkeypatch.setenv("LEAVEN_TEST_CALLS", str(calls))
        readback = load_rust_run_readback(run_dir, leaven_bin=fake)

    assert readback is not None
    assert readback.latest_checkpoint == "checkpoint_1"
    assert readback.graph.candidate_count == 2
    assert readback.graph.bytes == 128
    assert calls.read_text(encoding="utf-8").splitlines() == [
        "run",
        "inspect",
        "--run-dir",
        str(run_dir),
    ]


def test_load_rust_run_readback_reports_cli_failure(tmp_path: Path) -> None:
    run_dir = tmp_path / "run"
    (run_dir / "checkpoints").mkdir(parents=True)
    (run_dir / "checkpoints" / "LATEST").write_text("checkpoint_1\n", encoding="utf-8")
    fake = tmp_path / "leaven"
    fake.write_text("#!/bin/sh\necho nope >&2\nexit 7\n", encoding="utf-8")
    fake.chmod(0o755)

    with pytest.raises(RuntimeError, match="leaven run inspect failed"):
        load_rust_run_readback(run_dir, leaven_bin=fake)
